# Rime FrameForge Rime.Q 量化设计

## 1. 目的与边界

Rime.Q 是在 FP32 GPU 资源上仿真有限字长的统一数值契约。纹理、buffer 和节点内部计算仍使用 `f32`；量化只发生在声明的模块 **output port**，把值限制到指定的离散网格。它不把主链改成整数资源，也不声称仿真硬件内部的部分积、累加器或时序。

量化路径固定为：

```text
FP32 physical value
  -> ClipType transform（truncate / round / dither）
  -> fixed-point code
  -> representable-range clamp
  -> FP32 fixed-grid value
```

输出仍由 FP32 承载，但语义上只能取 `k × 2^-Y` 的值。profile 精度只能定义承载网格，不能创造 sensor 没有的有效信息。

## 2. Rime.Q profile

profile 统一写作 `uX.Y` 或 `sX.Y`，其中：

- `u` 表示 unsigned；没有 sign bit。
- `s` 表示 signed；sign bit 占 1 bit，表示数值的正负，不计入 `X`。
- `X` 是整数 magnitude bits，决定动态范围。
- `Y` 是小数 bits，决定精度。
- `LSB = 2^-Y`。

对值 `x`，理想 fixed-point code 为 `x × 2^Y`。可表示范围为：

```text
uX.Y: 0 <= x <= 2^X - 2^-Y
sX.Y: -2^X <= x <= 2^X - 2^-Y
```

因此 `u0.Y` 的范围是 `[0, 1 - 2^-Y]`，它不能表示负值，也不能精确表示 `1.0`；`s1.Y` 的范围是 `[-2, 2 - 2^-Y]`。`X` 增加一位，动态范围上限约翻倍；`Y` 增加一位，LSB 减半、精度提高一倍。实现应拒绝缺少 sign、dot、数字、负 bit 数，以及 `X + Y > 24`（无法保证 FP32 对 code 的精确承载）的 profile。

所有配置、序列化和 UI 只使用短格式 `uX.Y`/`sX.Y`，由同一份 Rust parser/formatter 产生；WGSL 不自行解析字符串。

## 3. 物理域与 sensor 位深

Sensor/DNG 的整数 code、black level offset 和 white level 只在 BLC 的输入适配边界使用。BLC 输出进入 Rime.Q physical domain 后：

```text
0      = black reference
1      = sensor saturation / white reference
x < 0  = below-black noise（仅在 signed 数据域允许）
x > 1  = saturation 以上的 headroom / HDR 信号
```

BLC 的当前默认输出是 **unsigned `u0.14`**，不再采用旧文档中“BLC 输出 signed normalized RAW”的说法。若确实需要 below-black residual，必须声明另一个明确的 signed profile 和数据域，不能把 `u0.14` 与 signed 语义混用。

Rime.Q 的 profile 与 sensor 有效位数是两个事实：profile 决定输出网格，输入 descriptor/metadata 决定其中多少 bit 有效。后续模块不得把填充位当作新信息。

### 3.1 10-bit sensor → `u0.14`

令 10-bit sensor code 为 `c10 ∈ {0, …, 1023}`，归一化到 `[0, 1)` 后映射到 `u0.14`：

```text
q14 = round((c10 / 1023) * (2^14 - 1))
value = q14 * 2^-14
```

`q14` 虽然占 14 个 fractional bit，但输入只有 10 个有效灰阶 bit；因此最低约 4 个输出 bit 没有独立的 sensor 信息（它们是填充/重映射产生的位）。将这 4 个 bit 的离散台阶通过乘法抬到显示高 8 位，会看见底部分层、结块或油画感。可在 output quantization 的末位使用 deterministic dither 打散边界、减轻 banding；**dither 不会恢复不存在的 4 bit 信息**。

若实现采用简单零扩展，等价表达为 `q14 = c10 << 4`（再按 profile 上限 clamp）；端点重映射只改变端点对齐，不改变“仅 10 个有效 bit”的事实。

### 3.2 14-bit sensor → `u0.14`

令 14-bit code 为 `c14 ∈ {0, …, 16383}`，按相同 physical domain 直接进入 `u0.14`：

```text
q14 = c14
value = q14 * 2^-14
```

此时 14 个 fractional bit 都可承载有效灰阶（在实际 black/white normalization 后，仍须以 descriptor 声明的有效范围为准）。如果希望保留暗部有效灰阶，必须用合适的 gain 或 gamma 曲线抬升低位信号，同时接受有限的高位精度重新分配；不能把 10-bit 与 14-bit 输入无条件套用同一条曲线。

## 4. 增益、headroom 与动态范围压缩

`u0.y` 只容纳小于 1 的 nominal signal。LSC/WBC 等全局乘因子可能产生：

```text
x ∈ u0.y,       z = g × x
1 < g×x < 2     -> 需要 u1.y headroom
2 <= g×x < 4    -> 需要 u2.y headroom
```

`u1.y` 的上限约为 2，`u2.y` 的上限约为 4；超出 profile 上限仍必须按 clamp policy 处理。若 `z > 1` 只是过增益且没有物理意义，可显式 Clip/饱和。若 `z > 1` 是真实高光或 HDR 信号，不能靠 dither，也不能直接 clip；必须先做动态范围压缩，再量化到 nominal domain。

一种线性示例是把 `u1.y` 的 `[0, 2)` 压到 `u0.(y+1)`：

```text
c = z / 2                    # c ∈ [0, 1)
q = round(c × 2^(y+1))
compressed = q × 2^-(y+1)
```

实际 tone mapping 可使用非线性 `c = C(z)`（tone/gamma/shoulder），但必须明确 `C` 的输入范围、单调性和高光保留目标。适当 gain、dynamic-range compression、tone 或 gamma 是保留**有效信号**并把它映射到显示高 8 位的手段；dither 只处理量化造成的离散伪影。

结论：

- 无效低位或量化边界造成的 banding/结块：可用 deterministic dither 打散，不会增加信息。
- 有效但超出 nominal range 的信号：用 compression/tone/gamma/appropriate gain 保留，再决定是否饱和。
- 只修改 `uX.Y` 而不同时检查 sensor effective bits、gain 和曲线，会把物理问题伪装成精度问题。

## 5. Graph 与 module 配置

Rime.Q 提供 graph 总开关和每个模块的 output 配置。Rust 是配置 defaults、profile 合法性和 effective state 的唯一 authority；React 只编辑并展示该 contract，WGSL 只执行已验证的数值计划。

### 5.1 配置层级

```text
GraphQuantizationConfig {
    graph_id
    enabled
    modules: Map<NodeId, ModuleQuantizationConfig>
}

ModuleQuantizationConfig {
    output_enabled
    output_profile: uX.Y | sX.Y
    dither_enabled
    clip_type
}
```

模块的 `enable/disable` 与 `bypass` 来自 graph presentation，只读，不是量化配置。Inspector 树从 presentation 的 `parent_id` 递归构建，保留 graph、VFE/VBE/VPE、pass 和 disabled/bypass 模块；不得在前端另写模块清单。

### 5.2 Output-only 与输入继承

本契约只配置模块 output，不提供独立 input profile。模块输入的 effective profile/domain 继承上游模块的 output；因此每个 output quantization point 都是下一模块的输入边界：

```text
source descriptor -> BLC output -> WBC input
WBC output        -> DEM input
DEM output        -> RGB2YUV input
```

第一个模块的输入由 sensor descriptor/前置资源声明。若上游没有量化，输入保持上游的 FP32 physical domain。应用配置时必须拒绝未知 module、非法 profile 或不匹配的 input/output domain，不能静默回退。

### 5.3 Precedence 与 effective state

保存的 module preference 与运行时 effective state 分离。优先级严格为：

```text
if graph.enabled == false:
    effective_output = false
    effective_dither = false
else if module is disabled or bypass:
    effective_output = false
    effective_dither = false
else:
    effective_output = saved.output_enabled
    effective_dither = saved.output_enabled && saved.dither_enabled
```

关闭 graph 总开关只强制关闭 effective output/dither，不清除 profile、ClipType 或 module 偏好；重新打开后，enabled 模块恢复保存的偏好。被强制关闭的控件显示 disabled。运行中按现有 stopped/completed 配置门禁拒绝编辑。

## 6. ClipType、dither 与 clamp

`ClipType` 是 output quantization 的可序列化规则，至少包括：

```text
truncate: floor(x × 2^Y)
round:    floor(x × 2^Y + 0.5)
dither:   floor((x + d × 2^-Y) × 2^Y)
```

其中 `d` 是确定性的、受 seed/stream/key 约束的 signed dither；它只作用于 profile 的末位 bit。`x == 0` 不注入非零 dither。负数 round 遵循上述 `floor(value + 0.5)` 语义，不得由另一套“对称 round”替换。

统一执行顺序不可改变：

```text
1. 按 ClipType 对 physical value 做 transform
2. 转为整数 fixed-point code
3. 按 profile qmin/qmax 做 saturation/clamp
4. 以 code × 2^-Y 写回 FP32 fixed-grid value
```

默认 saturation 是 clamp；不支持 wrap-around。NaN 和 Infinity 必须报错，不能静默 clamp。dither 的职责是打散无效的量化边界，不是压缩动态范围，也不是恢复 sensor 信息。

## 7. 默认 output profile

当前 Normal Graph 的精确默认值：

| Module | Default output profile |
| --- | --- |
| BLC | `u0.14` |
| WBC | `u0.12` |
| DEM | `u0.12` |
| RGB2YUV | `u0.10` |

这些值只定义各 output 的承载网格；输入仍按上一 output 继承。BLC 的 `u0.14` 是 unsigned contract，不能解释为 signed RAW。

## 8. Rust / WGSL 一致性要求

Rust `rime-quant` 实现 profile parse/format、LSB、范围、ClipType、dither 和错误边界；graph config 由 Rust 生成 defaults 并解析 effective precedence。WGSL helper 必须使用相同的：

- `uX.Y`/`sX.Y` 范围和 sign-bit 解释；
- ClipType transform、负数规则、zero-dither 规则；
- transform → code → clamp → FP32 的顺序；
- 非有限输入和不支持的 saturation 行为；
- deterministic dither 的 seed/stream/key 语义。

不得在 Rust 和 WGSL 之间复制两套 profile 解析、clamp 或 dither 定义。配置 revision 在执行引擎接入时用于重建量化计划；在没有真实 output QuantPoint 的 graph 上，UI 只能报告配置已验证/已传递，不能虚报 GPU 已执行量化。
