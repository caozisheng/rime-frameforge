<!-- markdownlint-disable MD013 -->

# Graph Rime.Q Configuration Design

## Goal

为每个 graph 提供统一的顶层 Rime.Q 定标开关和按模块配置，并让未选择节点时的 Node inspector 以 tree-view 展示 graph、VFE/VBE/VPE、pass-1/2/3 及模块配置。未来新增 graph 复用相同 contract 和 Inspector 机制。

## Scope and graph lifecycle

Minimal Graph 已移除。本设计面向当前 Normal Graph，并作为未来 graph 的通用模式：用户先选择 graph；未选择任何节点时，Inspector 显示所选 graph 的顶层配置；选择节点后，Inspector 切换到节点配置。graph presentation 继续由 Rust 生成，是模块层级、enable/bypass/disable 状态和 pass 层级的事实源。

Normal Graph 当前仍是 read-only presentation。此次改动必须建立真实的配置 contract、UI 编辑路径和 runtime command 边界；在实际执行引擎接入前，UI 不得宣称已改变 GPU 数值。

## Inspector tree

无节点选中时显示：

```text
整体
├─ Rime.Q 定标
VFE
├─ sensor correction
│  ├─ RAW Source
│  ├─ BLC
│  └─ ...
VBE
├─ WBC
├─ DEM
└─ ...
VPE
├─ pass-1
│  ├─ ...
├─ pass-2
│  ├─ ...
└─ pass-3
   ├─ ...
```

树节点来自 graph presentation 的 `parent_id`，不得在 React 中再次手写模块列表。group、pass 和 operator 都保留，disabled/bypass 模块也展示，以便用户理解不可配置原因。

### Graph-level controls

根节点“整体”提供：

- `Rime.Q 定标`：控制整个 graph 的总开关。
- 开关关闭时，所有模块的 **effective output quantization** 和 **effective dither** 都为关闭。
- 模块保存的用户偏好不被清除；再次打开总开关后，enabled 模块恢复各自偏好。

### Module-level controls

每个模块节点提供：

- `enable/disable`：只读，取自 graph presentation。
- `bypass`：只读，取自 graph presentation。
- `output Rime.Q`：可配置的 output quantization 开关。
- `output Rime.Q profile`：仅 output 开启时可配置。
- `output dither`：仅 output 开启且模块为 enabled 时可配置。
- `ClipType`：可配置，作用于 output 定标；dither 仍针对 profile 的末位 bit。

一般 input 定标沿用前级 output，因此不提供独立 input 定标控件。模块的 effective 状态按以下优先级计算：

```text
if graph.enabled == false:
    effective_output = false
    effective_dither = false
else if module.mode in {disabled, bypass}:
    effective_output = false
    effective_dither = false
else:
    effective_output = module.output_enabled
    effective_dither = module.output_enabled && module.dither_enabled
```

被强制关闭的控件显示 disabled，但用户保存的 `output_enabled`、`dither_enabled`、profile 和 ClipType 保留。运行中沿用现有 stop/completed 配置门禁，控件不可编辑。

## Shared configuration contract

Rust 定义并生成 graph config defaults；React 只消费生成的 contract 和 presentation。逻辑模型为：

```text
GraphQuantizationConfig {
    graph_id: GraphId,
    enabled: bool,
    modules: Map<NodeId, ModuleQuantizationConfig>,
}

ModuleQuantizationConfig {
    mode: enabled | bypass | disabled, // derived, read-only
    output_enabled: bool,
    output_profile: RimeQProfile,
    dither_enabled: bool,
    clip_type: ClipType,
}
```

`mode` 不由 UI 写入，而由 graph presentation 派生并在 runtime 校验。graph config 必须只包含 graph 中已知的 output port/module；未知 module、非法 profile 或不匹配的 input/output domain 拒绝应用，不静默回退。

配置变更通过 `set_graph_quantization` 和 `set_module_quantization` runtime command 传递给 authority。配置 revision/numeric revision 需要在执行引擎接入时递增并用于重建量化计划；当前 read-only Normal Graph 至少必须保持 UI、contract 和命令边界一致。

## Rime.Q notation

统一使用 `uX.Y` 或 `sX.Y`：

- `u` 表示 unsigned，不包含符号 bit。
- `s` 表示 signed，符号占 1 bit。
- `X` 是整数部分 bit 数，决定可表示动态范围。
- `Y` 是小数部分 bit 数，决定精度。
- LSB 为 `2^-Y`。

Profile 的字符串格式是唯一 UI/序列化显示格式，不再使用旧的 `UQ`/`SQ` 长格式。signed profile 的范围和 unsigned profile 的范围必须由同一 parser/formatter 实现，禁止前端另写解析规则。

## Sensor bit depth and physical meaning

Rime.Q profile 描述承载网格，不会创造 sensor 没有的信息。sensor 输入有效位数必须来自输入 descriptor/metadata，是只读事实；它与 BLC output profile 的关系要在 Inspector 和文档中可见。

### 10-bit sensor to `u0.14`

假定 sensor 输入像素为 10-bit，而 BLC 输出 `u0.14`：输入只有 10 个有效灰阶 bit，输出的 14 个 fractional bit 中最低 4 bit 是无效填充位。后续如果某个算法环节把这 4 个 bit 通过乘法抬到显示高 8 位，离散台阶会被直接看见，产生底部分层、结块或油画感。通常在 output quantization 的末位使用 deterministic dither 打散量化边界，改善视觉瑕疵；dither 不会恢复不存在的信息。

### 14-bit sensor to `u0.14`

假定 sensor 输入像素为 14-bit，而 BLC 输出 `u0.14`：14 个 fractional bit 都是有效灰阶。如果有效灰阶或动态范围需要保留，后续环节需要通过乘法抬升这些低位信号，例如抬高 gamma 曲线，同时接受高位精度重新分配。曲线、增益和 sensor 有效位数强相关，不能把同一曲线无条件用于不同 sensor bit depth。

### Gain/headroom after `u0.y`

sensor 整数被量化到 `u0.y` 后，LSC、WBC 等全局乘因子会使中间数据进入 `u1.y` 甚至 `u2.y`，也就是出现大于 1.0 的 headroom。若这些超出 saturation level 的值没有物理意义，可以按明确 ClipType/saturation policy clip；若它们携带真实高光信号，必须先做 dynamic-range compression，例如从 `u1.y` 压缩到 `u0.(y+1)`，再由 tone/gamma/自适应增益模块恢复并保持到显示高 8 位。

因此：

- 无效信号用 dither 消除结块和 banding，不能用 dither 恢复信息。
- 有效信号用 dynamic-range compression 和自适应 tone/gamma/增益保留信息，再映射到显示高 8 位。
- `uX.Y` 的选择、增益、曲线和 sensor effective bits 必须作为一组设计；不能只修改 profile 而忽略物理信息路径。

## ClipType and quantization

`ClipType` 是 output 定标的简洁、可序列化规则。基线至少包括：

- `truncate`：`floor(x * 2^Y)`。
- `round`：`floor(x * 2^Y + 0.5)`，保持现有 reference 的负数语义。
- `dither`：按 profile LSB 加 deterministic signed dither 后 floor；正负值方向与零点规则固定，`x == 0` 不注入非零信号。

统一顺序：

```text
ClipType transform
    -> fixed-point code
    -> representable range saturation/clamp
    -> FP32 fixed-grid value
```

NaN 和 Infinity 必须拒绝，不能静默 clamp。默认 saturation 为 clamp；wrap-around 不在本阶段支持。Rust 与 WGSL 必须使用同一规则。

## Default output profiles

当前 graph 默认 output Rime.Q：

| Module | Default output |
| --- | --- |
| BLC | `u0.14` |
| WBC | `u0.12` |
| DEM | `u0.12` |
| RGB2YUV | `u0.10` |

新要求优先于旧文档中 BLC 的 signed normalized RAW 表述。BLC output contract 采用 `u0.14`；若后续需要表达 below-black signed residual，必须另设明确的 signed profile 和数据域，不能把 `u0.14` 与 signed 语义混用。

## `rime-quant` rewrite

`crates/rime-quant` 以新 contract 为核心重写，删除会造成第二套语义的旧 profile 表达：

- 提供 `uX.Y`/`sX.Y` 的 parse/format、合法性校验、LSB、范围计算。
- 提供 `ClipType`，统一 truncate/round/dither 语义。
- 保留 deterministic dither 的稳定 stream/seed/key 行为，但让 dither 明确作用于 profile 的末位 bit。
- 继续使用 FP32 carrier；输出保持 FP32 fixed-grid value。
- 校验 `X + Y <= 24` 的 exact FP32 grid 限制；非有限输入、非法 profile、缺失 dither 配置和不支持的 saturation 返回明确错误。
- WGSL helper 与 Rust 实现共享枚举语义和执行顺序。

测试必须覆盖 notation round-trip、signed/unsigned bounds、LSB、负数 truncate/round、dither zero、边界 saturation、non-finite、exact-grid limit 和 deterministic sequence。

## Verification

1. Rust graph/manifest tests 验证 graph config schema、树层级和四个默认 profile。
2. Rust quant tests 验证新 profile notation、ClipType、dither 和错误边界。
3. React tests 验证无节点选择显示 graph tree，树中包含 VFE/VBE/VPE/pass-1/2/3，global off 覆盖 effective state，disabled/bypass 控件 disabled，global reopen 恢复模块偏好。
4. 运行 TypeScript、Vitest、Cargo tests/clippy、WASM/Vite build。
5. 浏览器 smoke 验证选择 graph、清空 node selection、打开 graph inspector、展开 module、切换全局和模块控件状态联动。
