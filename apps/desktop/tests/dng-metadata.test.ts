import { describe, expect, it } from 'vitest';

import {
  DEFAULT_DNG_FONT_SIZE,
  MAX_DNG_FONT_SIZE,
  MIN_DNG_FONT_SIZE,
  buildDngMetadataGroups,
  clampDngFontSize,
  resolveDngTagName,
} from '../src/components/dng-metadata.js';
import type { DngFrameDescriptor, DngSequenceDescriptor } from '../src/runtime/worker-bridge.js';

const descriptor = {
  frameIndex: 0,
  fileName: 'frame-000.dng',
  width: 2,
  height: 2,
  rowStrideSamples: 2,
  storageBits: 14,
  cfa: 'rggb',
  blackLevel: 64,
  whiteLevel: 16383,
  dngVersion: [1, 6, 0, 0],
  backwardVersion: [1, 4, 0, 0],
  cameraModel: 'Test Camera',
  metadataHash: 'metadata',
  rawDigest: 'raw',
  whiteBalanceGains: [2, 1, 4],
  metadata: {
    dngVersion: [1, 6, 0, 0], backwardVersion: [1, 4, 0, 0], blackRepeat: [2, 2], blackLevels: [64, 64, 64, 64], blackDeltaH: null, blackDeltaV: null, whiteLevels: [16383], linearizationTable: null, cameraModel: 'Test Camera', colorMatrix1: [1, 2, 3, 4, 5, 6, 7, 8, 9], calibrationIlluminant1: 'D65', asShotNeutral: [0.5, 1, 0.75], asShotWhiteXY: null, colorMatrix2: null, cameraCalibration1: null, cameraCalibration2: null, forwardMatrix1: null, forwardMatrix2: null, analogBalance: null, baselineExposure: null, profileName: null, exifExposureTime: null, exifFNumber: null, exifIsoSpeed: null, exifBrightnessValue: null, exifExposureBiasValue: null, exifDateTimeOriginal: null, exifFocalLength: null, xmpByteLength: null, iptcByteLength: null, iccByteLength: null, newRawImageDigest: null, ifd0Extra: [], rawExtra: [], exifExtra: [],
  },
} satisfies DngFrameDescriptor;

const sequence: DngSequenceDescriptor = {
  directory: 'C:/frames',
  paths: ['C:/frames/frame1.dng', 'C:/frames/frame2.dng', 'C:/frames/frame10.dng'],
  fileNames: ['frame1.dng', 'frame2.dng', 'frame10.dng'],
  frameCount: 3,
};

describe('DNG metadata tree model', () => {
  it('clamps metadata font size to the supported range', () => {
    expect(clampDngFontSize(0)).toBe(MIN_DNG_FONT_SIZE);
    expect(clampDngFontSize(20)).toBe(MAX_DNG_FONT_SIZE);
    expect(clampDngFontSize(undefined)).toBe(DEFAULT_DNG_FONT_SIZE);
  });

  it('builds stable semantic groups with required defaults', () => {
    const groups = buildDngMetadataGroups(descriptor);
    expect(groups.map((group) => group.id)).toEqual(['runtime', 'frame', 'image', 'sensor', 'calibration', 'exposure', 'lens', 'exif', 'tiff', 'dng', 'opcodes', 'integrity']);
    expect(groups.find((group) => group.id === 'runtime')?.defaultExpanded).toBe(true);
    expect(groups.find((group) => group.id === 'calibration')?.defaultExpanded).toBe(false);
    expect(groups.find((group) => group.id === 'frame')?.children.map((child) => child.label)).toContain('File name');
    expect(groups.find((group) => group.id === 'calibration')?.children.map((child) => child.label)).toContain('As shot white xy');
  });
  it('builds a sequence group with ordered filenames and current position', () => {
    const groups = buildDngMetadataGroups(descriptor, sequence, 1);
    const sequenceGroup = groups.find((group) => group.id === 'sequence');
    expect(sequenceGroup?.children.map((child) => child.label)).toEqual(['Directory', 'File count', 'Current frame', 'Current file', 'Files']);
    expect(sequenceGroup?.children.find((child) => child.label === 'Current frame')?.value).toBe('2 / 3');
    expect(sequenceGroup?.children.find((child) => child.label === 'Files')?.children?.map((child) => child.value)).toEqual(sequence.fileNames);
  });
  it('resolves standard tags with TIFF-first precedence and unknown fallback', () => {
    expect(resolveDngTagName(256)).toBe('ImageWidth');
    expect(resolveDngTagName(50706)).toBe('DNGVersion');
    expect(resolveDngTagName(65535)).toBeUndefined();
  });

  it('uses resolved names for extra metadata tag labels', () => {
    const groups = buildDngMetadataGroups({
      ...descriptor,
      metadata: {
        ...descriptor.metadata,
        ifd0Extra: [{ tag: 256, fieldType: 'Long', count: 1, value: '2' }],
        exifExtra: [{ tag: 50706, fieldType: 'Byte', count: 4, value: '[1, 6, 0, 0]' }],
        rawExtra: [{ tag: 65535, fieldType: 'Long', count: 1, value: 'unknown' }],
      },
    });
    expect(groups.find((group) => group.id === 'tiff')?.children[0]?.label).toBe('ImageWidth (256)');
    expect(groups.find((group) => group.id === 'exif')?.children.find((child) => child.label === 'DNGVersion (50706)')?.label).toBe('DNGVersion (50706)');
    expect(groups.find((group) => group.id === 'tiff')?.children[1]?.label).toBe('Tag 65535');
  });


  it('labels every standard DNG opcode id', () => {
    const standardOpcode = {
      id: 14,
      specVersion: [1, 6, 0, 0],
      flags: 0,
      parameterLength: 0,
      parametersHex: '',
      warpRectilinear: null,
      fixVignetteRadial: null,
    };
    const groups = buildDngMetadataGroups({
      ...descriptor,
      metadata: { ...descriptor.metadata, opcodeList3: [standardOpcode] },
    });

    expect(groups.find((group) => group.id === 'opcodes')?.children[2]?.children?.[0]?.label)
      .toBe('WarpRectilinear2 (14) [0]');
  });
  it('renders all opcode lists with raw and structured parameters', () => {
    const groups = buildDngMetadataGroups({
      ...descriptor,
      metadata: {
        ...descriptor.metadata,
        opcodeList1: [{
          id: 65000,
          specVersion: [1, 4, 0, 0],
          flags: 3,
          parametersHex: 'deadbeef',
          warpRectilinear: null,
          fixVignetteRadial: null,
        }],
        opcodeList2: [{
          id: 3,
          specVersion: [1, 3, 0, 0],
          flags: 0,
          parameterLength: 56,
          parametersHex: '00'.repeat(56),
          warpRectilinear: null,
          fixVignetteRadial: {
            coefficients: [0.1, 0.02, 0.003, 0.0004, 0.00005],
            opticalCenter: [0.45, 0.55],
          },
        }],
        opcodeList3: [{
          id: 1,
          specVersion: [1, 3, 0, 0],
          flags: 1,
          parametersHex: '00000001',
          warpRectilinear: {
            coefficientSets: [{
              radial: [1, 0.01, 0.001, 0],
              tangential: [0.0001, 0.00001],
            }],
            opticalCenter: [0.49, 0.51],
          },
            fixVignetteRadial: null,
        }],
      },
    } as unknown as DngFrameDescriptor);

    const opcodes = groups.find((group) => group.id === 'opcodes');
    expect(opcodes?.children.map((child) => child.label)).toEqual([
      'OpcodeList1',
      'OpcodeList2',
      'OpcodeList3',
    ]);
    expect(opcodes?.children[0]?.summary).toBe('1 opcode');
    expect(opcodes?.children[1]?.summary).toBe('1 opcode');
    expect(opcodes?.children[0]?.children?.[0]?.label).toBe('Opcode 65000 [0]');
    expect(opcodes?.children[0]?.children?.[0]?.children?.find((child) => child.label === 'Raw parameters')?.children?.[0]?.value).toBe('deadbeef');
    expect(opcodes?.children[2]?.children?.[0]?.label).toBe('WarpRectilinear (1) [0]');
    expect(opcodes?.children[2]?.children?.[0]?.children?.find((child) => child.label === 'Flags')?.summary).toBe('optional');
    expect(opcodes?.children[2]?.children?.[0]?.children?.find((child) => child.label === 'WarpRectilinear')?.children?.find((child) => child.label === 'Coefficient sets')?.summary).toBe('1 set');
    expect(opcodes?.children[1]?.children?.[0]?.children?.find((child) => child.label === 'FixVignetteRadial')?.children?.find((child) => child.label === 'Coefficients')?.summary).toBe('[0.1, 0.02, 0.003, 0.0004, 0.00005]');
    expect(opcodes?.children[1]?.children?.[0]?.children?.find((child) => child.label === 'FixVignetteRadial')?.children?.find((child) => child.label === 'Optical center')?.summary).toBe('[0.45, 0.55]');
  });
  it('renders missing optional metadata arrays as unavailable', () => {
    const incomplete = {
      ...descriptor,
      metadata: {
        ...descriptor.metadata,
        asShotWhiteXY: undefined,
        linearizationTable: undefined,
        exifExtra: undefined,
      },
    } as unknown as DngFrameDescriptor;

    expect(() => buildDngMetadataGroups(incomplete)).not.toThrow();
    const calibration = buildDngMetadataGroups(incomplete).find((group) => group.id === 'calibration');
    expect(calibration?.children.find((child) => child.label === 'As shot white xy')?.value).toBe('—');
  });
});
