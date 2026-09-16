import type { DngFrameDescriptor, DngOpcodeDescriptor, DngRawTagDescriptor } from '../runtime/worker-bridge.js';
import { resolveDngTagName } from './dng-tag-names.js';
export { resolveDngTagName } from './dng-tag-names.js';

export const MIN_DNG_FONT_SIZE = 9;
export const MAX_DNG_FONT_SIZE = 13;
export const DEFAULT_DNG_FONT_SIZE = 11;

export interface DngTreeNode {
  readonly id: string;
  readonly label: string;
  readonly value?: string;
  readonly summary?: string;
  readonly children?: readonly DngTreeNode[];
}

export interface DngMetadataGroup extends DngTreeNode {
  readonly children: readonly DngTreeNode[];
  readonly defaultExpanded: boolean;
}

export function clampDngFontSize(value: number | undefined): number {
  if (value === undefined || !Number.isFinite(value)) return DEFAULT_DNG_FONT_SIZE;
  return Math.min(MAX_DNG_FONT_SIZE, Math.max(MIN_DNG_FONT_SIZE, Math.round(value)));
}

function leaf(id: string, label: string, value: unknown): DngTreeNode {
  return { id, label, value: formatValue(value) };
}

function arrayNode(id: string, label: string, values: readonly unknown[] | null | undefined, summary?: string): DngTreeNode {
  if (values === null || values === undefined) return leaf(id, label, null);
  return {
    id,
    label,
    summary: summary ?? `[${values.map(formatValue).join(', ')}]`,
    children: values.map((value, index) => leaf(`${id}.${index}`, `[${index}]`, value)),
  };
}

function rationalNode(id: string, label: string, value: readonly number[] | null, unit = ''): DngTreeNode {
  if (value === null || value.length < 2) return leaf(id, label, null);
  const numerator = value[0] ?? 0;
  const denominator = value[1] ?? 0;
  const decimal = denominator === 0 ? '0' : (numerator / denominator).toString();
  return arrayNode(id, label, value, `${decimal}${unit}`);
}

function tagNodes(prefix: string, tags: readonly DngRawTagDescriptor[] | null | undefined): readonly DngTreeNode[] {
  return (tags ?? []).map((tag, index) => {
    const name = resolveDngTagName(tag.tag);
    const label = name === undefined ? `Tag ${tag.tag}` : `${name} (${tag.tag})`;
    return {
      id: `${prefix}.${tag.tag}.${index}`,
      label,
      summary: `${tag.fieldType} × ${tag.count}`,
      children: [
        leaf(`${prefix}.${tag.tag}.${index}.type`, 'Field type', tag.fieldType),
        leaf(`${prefix}.${tag.tag}.${index}.count`, 'Count', tag.count),
        leaf(`${prefix}.${tag.tag}.${index}.value`, 'Value', tag.value),
      ],
    };
  });
}

const OPCODE_NAMES: Readonly<Record<number, string>> = {
  1: 'WarpRectilinear',
  2: 'WarpFisheye',
  3: 'FixVignetteRadial',
  4: 'FixBadPixelsConstant',
  5: 'FixBadPixelsList',
  6: 'TrimBounds',
  7: 'MapTable',
  8: 'MapPolynomial',
  9: 'GainMap',
  10: 'DeltaPerRow',
  11: 'DeltaPerColumn',
  12: 'ScalePerRow',
  13: 'ScalePerColumn',
  14: 'WarpRectilinear2',
};

function opcodeFlags(flags: number): string {
  const names = [
    ...(flags & 1 ? ['optional'] : []),
    ...(flags & 2 ? ['preview-skip'] : []),
  ];
  const unknown = flags & ~3;
  if (unknown !== 0) names.push(`unknown 0x${unknown.toString(16)}`);
  return names.length === 0 ? 'required' : names.join(', ');
}

function rawParameterNodes(id: string, hex: string): readonly DngTreeNode[] {
  if (hex.length === 0) return [leaf(`${id}.empty`, '[empty]', '')];
  const chunks: DngTreeNode[] = [];
  for (let offset = 0; offset < hex.length; offset += 64) {
    const byteStart = offset / 2;
    const value = hex.slice(offset, offset + 64);
    chunks.push(leaf(`${id}.${byteStart}`, `[${byteStart.toString(16).padStart(4, '0')}]`, value));
  }
  return chunks;
}

function opcodeNode(list: number, opcode: DngOpcodeDescriptor, index: number): DngTreeNode {
  const id = `opcodes.${list}.${index}`;
  const name = OPCODE_NAMES[opcode.id];
  const children: DngTreeNode[] = [
    leaf(`${id}.id`, 'Opcode ID', opcode.id),
    leaf(`${id}.version`, 'DNG version', formatVersion(opcode.specVersion)),
    { id: `${id}.flags`, label: 'Flags', value: formatValue(opcode.flags), summary: opcodeFlags(opcode.flags) },
    leaf(`${id}.length`, 'Parameter length', `${opcode.parameterLength} bytes`),
  ];
  if (opcode.warpRectilinear !== null && opcode.warpRectilinear !== undefined) {
    const sets = opcode.warpRectilinear.coefficientSets ?? [];
    children.push({
      id: `${id}.warpRectilinear`,
      label: 'WarpRectilinear',
      children: [
        {
          id: `${id}.warpRectilinear.sets`,
          label: 'Coefficient sets',
          summary: `${sets.length} ${sets.length === 1 ? 'set' : 'sets'}`,
          children: sets.map((set, setIndex) => ({
            id: `${id}.warpRectilinear.sets.${setIndex}`,
            label: `Plane [${setIndex}]`,
            children: [
              arrayNode(`${id}.warpRectilinear.sets.${setIndex}.radial`, 'Radial', set.radial),
              arrayNode(`${id}.warpRectilinear.sets.${setIndex}.tangential`, 'Tangential', set.tangential),
            ],
          })),
        },
        arrayNode(`${id}.warpRectilinear.center`, 'Optical center', opcode.warpRectilinear.opticalCenter),
      ],
    });
  }
  children.push({
    id: `${id}.raw`,
    label: 'Raw parameters',
    summary: `${opcode.parameterLength} bytes`,
    children: rawParameterNodes(`${id}.raw`, opcode.parametersHex),
  });
  return {
    id,
    label: name === undefined ? `Opcode ${opcode.id} [${index}]` : `${name} (${opcode.id}) [${index}]`,
    children,
  };
}

function opcodeListNode(list: number, opcodes: readonly DngOpcodeDescriptor[] | null | undefined): DngTreeNode {
  const values = opcodes ?? [];
  return {
    id: `opcodes.${list}`,
    label: `OpcodeList${list}`,
    summary: `${values.length} ${values.length === 1 ? 'opcode' : 'opcodes'}`,
    children: values.map((opcode, index) => opcodeNode(list, opcode, index)),
  };
}

function formatVersion(value: readonly number[] | null): string {
  return value === null ? '—' : value.join('.');
}

export function formatValue(value: unknown): string {
  if (value === null || value === undefined || value === '') return '—';
  if (typeof value === 'number') return Number.isInteger(value) ? value.toString() : value.toFixed(6).replace(/0+$/, '').replace(/\.$/, '');
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return `[${value.map(formatValue).join(', ')}]`;
  return String(value);
}

export function buildDngMetadataGroups(
  descriptor: DngFrameDescriptor,
  sequence: { readonly directory: string; readonly fileNames: readonly string[]; readonly frameCount: number } | null = null,
  sequenceIndex = descriptor.frameIndex,
): readonly DngMetadataGroup[] {
  const metadata = descriptor.metadata;
  const group = (id: string, label: string, defaultExpanded: boolean, children: readonly DngTreeNode[]): DngMetadataGroup => ({
    id, label, defaultExpanded, children,
  });

  return [
    ...(sequence === null ? [] : [group('sequence', 'Sequence', true, [
      leaf('sequence.directory', 'Directory', sequence.directory),
      leaf('sequence.fileCount', 'File count', sequence.frameCount),
      leaf('sequence.currentFrame', 'Current frame', `${sequenceIndex + 1} / ${sequence.frameCount}`),
      leaf('sequence.currentFile', 'Current file', descriptor.fileName),
      { id: 'sequence.files', label: 'Files', summary: `${sequence.fileNames.length} files`, children: (sequence.fileNames ?? []).map((fileName, index) => leaf(`sequence.files.${index}`, `[${String(index + 1).padStart(3, '0')}]`, fileName)) },
    ])]),
    group('runtime', 'Runtime', true, [
      leaf('runtime.frameIndex', 'Frame index', descriptor.frameIndex),
    ]),
    group('frame', 'Frame', true, [
      leaf('frame.fileName', 'File name', descriptor.fileName),
      leaf('frame.cameraModel', 'Camera model', descriptor.cameraModel),
    ]),
    group('image', 'Image', true, [
      leaf('image.width', 'Width', descriptor.width),
      leaf('image.height', 'Height', descriptor.height),
      leaf('image.rowStrideSamples', 'Row stride samples', descriptor.rowStrideSamples),
      leaf('image.sampleCount', 'Sample count', descriptor.rowStrideSamples * descriptor.height),
      leaf('image.storageBits', 'Storage bits', descriptor.storageBits),
    ]),
    group('sensor', 'Sensor / CFA', true, [
      leaf('sensor.cfa', 'CFA pattern', descriptor.cfa.toUpperCase()),
      arrayNode('sensor.blackRepeat', 'Black repeat', metadata.blackRepeat),
      arrayNode('sensor.blackLevels', 'Black levels', metadata.blackLevels),
      arrayNode('sensor.blackDeltaH', 'Black delta H', metadata.blackDeltaH),
      arrayNode('sensor.blackDeltaV', 'Black delta V', metadata.blackDeltaV),
      arrayNode('sensor.whiteLevels', 'White levels', metadata.whiteLevels),
      arrayNode('sensor.linearizationTable', 'Linearization table', metadata.linearizationTable, metadata.linearizationTable === null || metadata.linearizationTable === undefined ? undefined : `${metadata.linearizationTable.length} entries`),
    ]),
    group('calibration', 'Calibration', false, [
      leaf('calibration.illuminant1', 'Illuminant 1', metadata.calibrationIlluminant1),
      arrayNode('calibration.colorMatrix1', 'Color matrix 1', metadata.colorMatrix1, '3 × 3'),
      arrayNode('calibration.asShotNeutral', 'As shot neutral', metadata.asShotNeutral),
      arrayNode('calibration.asShotWhiteXY', 'As shot white xy', metadata.asShotWhiteXY),
      arrayNode('calibration.colorMatrix2', 'Color matrix 2', metadata.colorMatrix2, metadata.colorMatrix2 === null ? undefined : '3 × 3'),
      arrayNode('calibration.cameraCalibration1', 'Camera calibration 1', metadata.cameraCalibration1, metadata.cameraCalibration1 === null ? undefined : '3 × 3'),
      arrayNode('calibration.cameraCalibration2', 'Camera calibration 2', metadata.cameraCalibration2, metadata.cameraCalibration2 === null ? undefined : '3 × 3'),
      arrayNode('calibration.forwardMatrix1', 'Forward matrix 1', metadata.forwardMatrix1, metadata.forwardMatrix1 === null ? undefined : '3 × 3'),
      arrayNode('calibration.forwardMatrix2', 'Forward matrix 2', metadata.forwardMatrix2, metadata.forwardMatrix2 === null ? undefined : '3 × 3'),
      arrayNode('calibration.analogBalance', 'Analog balance', metadata.analogBalance),
      leaf('calibration.profileName', 'Profile name', metadata.profileName),
    ]),
    group('exposure', 'Exposure', false, [
      rationalNode('exposure.time', 'Exposure time', metadata.exifExposureTime, ' s'),
      leaf('exposure.iso', 'ISO', metadata.exifIsoSpeed),
      leaf('exposure.brightnessValue', 'Brightness value', metadata.exifBrightnessValue),
      leaf('exposure.exposureBiasValue', 'Exposure bias value', metadata.exifExposureBiasValue),
      leaf('exposure.baseline', 'Baseline exposure', metadata.baselineExposure),
      leaf('exposure.dateTime', 'Date/time original', metadata.exifDateTimeOriginal),
    ]),
    group('lens', 'Lens', false, [
      rationalNode('lens.fNumber', 'F number', metadata.exifFNumber),
      rationalNode('lens.focalLength', 'Focal length', metadata.exifFocalLength, ' mm'),
    ]),
    group('exif', 'EXIF', false, [
      leaf('exif.xmpBytes', 'XMP bytes', metadata.xmpByteLength),
      leaf('exif.iptcBytes', 'IPTC bytes', metadata.iptcByteLength),
      leaf('exif.iccBytes', 'ICC bytes', metadata.iccByteLength),
      ...tagNodes('exif.extra', metadata.exifExtra),
    ]),
    group('tiff', 'TIFF', false, [
      ...tagNodes('tiff.ifd0', metadata.ifd0Extra),
      ...tagNodes('tiff.raw', metadata.rawExtra),
    ]),
    group('dng', 'DNG', false, [
      leaf('dng.version', 'DNG version', formatVersion(metadata.dngVersion)),
      leaf('dng.backwardVersion', 'Backward version', formatVersion(metadata.backwardVersion)),
    ]),
    group('opcodes', 'Opcodes', false, [
      opcodeListNode(1, metadata.opcodeList1),
      opcodeListNode(2, metadata.opcodeList2),
      opcodeListNode(3, metadata.opcodeList3),
    ]),
    group('integrity', 'Integrity', false, [
      leaf('integrity.metadataHash', 'Metadata hash', descriptor.metadataHash),
      leaf('integrity.rawDigest', 'RAW digest', descriptor.rawDigest),
      leaf('integrity.newRawImageDigest', 'New raw image digest', metadata.newRawImageDigest),
    ]),
  ];
}
