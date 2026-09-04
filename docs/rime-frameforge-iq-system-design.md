<!-- markdownlint-disable MD013 -->

# Rime FrameForge IQ System Design

## 1. 文档目的

本文设计 Rime FrameForge 的 Image Quality（IQ）系统，并以 VBE `demosaic` 模块的 AHD method `04` 为第一个完整案例。

AHD 当前接收两个可调效果参数：

```text
ahd_l_threshold
ahd_c_threshold_sq
```

它们分别控制 Lab 空间中的亮度同质性阈值和平方色度差阈值。本文覆盖：

- DNG/EXIF 元数据到场景状态的建模；
- 标准 APEX EV、场景物理亮度、曝光偏差和 ISO/gain 的语义分离；
- AHD IQ 表资产格式与查找/插值规则；
- 场景标签；
- 两个 AHD 参数的调制曲线；
- 与现有 `preprocess → compute → postprocess`、method manifest、revision 和 GPU 执行边界的衔接；
- 桌面端可视化调节界面；
- 验证、错误处理和后续扩展路线。

本文是整体设计，不改变当前运行时代码。实现时必须继续遵守 `docs/rime-frameforge-top-architecture-design.md` 第 8 节 IQ contract。

## 2. 核心结论

### 2.1 三类量必须分轴

场景亮度、曝光偏差和 ISO/gain 不是同一个量，不能合并成单一 `EVscene` 轴：

| 量 | 物理/工程含义 | 主要 IQ 消费者 |
| --- | --- | --- |
| **场景亮度** | 拍摄时场景本身的 scene-referred 物理量，通常表示为亮度 `cd/m²`、照度 `lux` 或其 APEX/log 表示 | tone mapping、DRC、曝光相关影调策略；也可作为 AHD 的场景条件轴 |
| **曝光偏差 / 曝光补偿** | 相机实际成像曝光相对于测光基准或场景目标的偏离，正值通常表示增加曝光 | tone mapping 的场景还原和高光/暗部策略；不等于场景亮度 |
| **ISO / analog gain / digital gain** | 相机内部对 RAW 信号的放大状态，影响 RAW 信号幅度、读出噪声和 SNR | demosaic、RAW-NR、锐化等对噪声敏感的模块 |

因此，以下模型是错误的：

```text
aperture + shutter + ISO -> 一个场景亮度轴
```

正确模型是：

```text
scene brightness axis
exposure deviation axis
ISO / analog gain / digital gain axis
CCT axis
camera noise-profile axis
```

每个模块只选择真正影响自身效果的轴。IQ 系统保存完整 `SceneMeta`，但不能因为某个模块可以访问某个字段，就把该字段自动加入该模块的查表维度。

### 2.2 标准 EV 表用于亮度语义，不替代原始轴

用户指定的“Exposure value vs. Luminance (ISO 100, K = 12.5) and Illuminance (ISO 100, ...)”表应纳入系统，作为 EV 与物理亮度/照度之间的标准化换算层。

但该表不能把 ISO、曝光补偿和场景亮度混为一谈：

```text
DNG / external scene metadata
        |
        +--> scene brightness EV
        |        |
        |        +--> standard EV -> luminance / illuminance estimate
        |
        +--> exposure bias / deviation EV
        |
        +--> ISO / analog gain / digital gain
        |
        +--> CCT
```

标准 APEX 曝光值由光圈和曝光时间得到：

\[
EV_{100,capture}=\log_2\left(\frac{N^2}{t}\right)
\]

其中 `N` 为 f-number，`t` 为秒。这个值描述的是**相机采用的光圈/快门曝光组合在 ISO 100 基准下的曝光值**，不能直接宣称是场景物理亮度。

对于反射式测光，ISO 100 下常见亮度关系为：

\[
L=K\cdot2^{EV},\qquad K=12.5
\]

其中 `L` 的单位为 `cd/m²`。入射式照度使用独立常数：

\[
E=C\cdot2^{EV}
\]

其中 `E` 的单位为 `lux`，`C` 取决于采用的测光规范和校准；不能把 `K=12.5` 直接用于照度换算。

亮度/照度换算在系统中必须注明：

- `measured`：来自相机测光、外部测光或经过验证的传感器元数据；
- `calibrated`：由相机 profile 和测光标定模型转换得到；
- `estimated`：由曝光组合或不完整信息推断得到；
- `unavailable`：没有足够数据，不允许伪装成物理量。

### 2.3 AHD 首版采用二维基础表加独立调制曲线

AHD 首版推荐：

```text
base surface = scene brightness × ISO/noise state
modulation curves = optional exposure deviation / CCT / gain corrections
```

基础表表达 AHD 在不同场景亮度和 RAW 噪声状态下的主参数。曝光偏差主要服务 tone mapping；只有校准数据证明它改变 AHD 伪色或边缘选择时，才启用 AHD 的曝光偏差调制曲线。

这种设计避免：

- 用一个 EV 数值覆盖不同物理语义；
- 为 `scene brightness × ISO × exposure bias × CCT` 建立快速膨胀的稠密四维表；
- 把 ISO 的噪声影响误当作场景变暗；
- 在未来添加 CCT 时破坏首版资产格式。

## 3. 当前代码边界

### 3.1 AHD method

当前 `crates/rime-isp/src/vbe/dem/dem04.rs` 的 method manifest 为：

```text
module: dem
method: 04
shader: demosaic_ahd_main
parameters:
  cfa_pattern
  ahd_l_threshold
  ahd_c_threshold_sq
```

`dem04.wgsl` 在 `homogeneity()` 中执行：

```text
abs(center.L - neighbor.L) < ahd_l_threshold
(center.a - neighbor.a)^2 + (center.b - neighbor.b)^2 < ahd_c_threshold_sq
```

AHD 参数是当前 shader 的 Lab 域阈值，不是 RAW sensor code 域阈值。参数 schema 必须明确它们的值域、单位、上下限、Rime.Q 定标和是否存储平方值。

推荐保持：

```text
ahd_l_threshold      -> Lab ΔL threshold
ahd_c_threshold_sq   -> Lab Δa² + Δb² threshold, stored squared
```

`ahd_c_threshold_sq` 不在运行时开平方。直接保存平方阈值与当前 shader 的比较形式一致，并避免无意义的 `sqrt` 和单位歧义。

### 3.2 现有生命周期

AHD 参数派生必须在 `preprocess` 完成：

```text
SceneMeta[frame_index]
CameraMeta[camera_profile_id, calibration_revision]
AHD IQ table / graph override / style revision
        |
        v
lookup -> interpolate -> modulate -> validate -> Rime.Q quantize
        |
        v
ModuleParameterPacket[vbe.dem, method 04, frame_index, revision]
        |
        v
AHD compute
```

WGSL 不得自行读取 DNG metadata、IQ YAML、全局“当前亮度”或未版本化 cache。compute 只读取冻结后的 uniform/parameter packet。

AHD uniform 仍可保持当前 32-byte 布局：

```text
offset 0..15: cfa_pattern[4] as u32
offset 16:    vng_threshold slot / reserved threshold x
offset 20:    ahd_l_threshold
offset 24:    ahd_c_threshold_sq
offset 28:    reserved
```

具体字节布局由实现阶段的 shader binding contract 固定；IQ 系统不应把 uniform byte layout 当作 IQ 表 schema。

## 3.3 Crate 放置决策

APEX 计算、场景物理量换算和场景标签不应放进 `rime-isp`，也不应放进 `rime-dng`。它们是 source-neutral 的场景语义和元数据派生能力，不属于某个 GPU ISP operator，也不只服务 DNG 输入。

推荐新增独立 crate：

```text
crates/rime-scene/
```

`rime-scene` 负责：

- 标准 APEX EV 计算；
- EV 与亮度/照度的换算模型；
- 场景亮度、曝光偏差、ISO/gain 的 typed model；
- measured/calibrated/estimated/unavailable provenance；
- 场景标签规则和 label revision；
- 输入合法性、单位和缺失值校验；
- 不依赖 GPU、WGSL、DNG 解码器或桌面 UI 的纯函数。

依赖方向建议为：

```text
rime-core       <- graph primitives, revisions, generic metadata contracts
rime-scene      <- scene physics, APEX, provenance, labels
rime-dng        <- DNG decode and raw camera metadata
rime-iq         <- IQ assets, lookup, interpolation, modulation (future split)
rime-isp        <- ISP methods and preprocess; consumes rime-scene/rime-iq types
desktop / CLI   <- combines rime-dng + rime-scene + rime-iq + rime-isp
```

当前阶段不建议立即拆出 `rime-iq`，除非 IQ loader/interpolator 已经有多个模块共同使用。可以先把 IQ schema、AHD default table 和 `dem04` 的 preprocess glue 保持在 `rime-isp` 的 AHD asset boundary 内；当第二个模块复用 lookup、曲线或 style resolver 时，再提取无 ISP 依赖的 `rime-iq`。

### 3.3.1 为什么不是 `rime-dng`

`rime-dng` 的职责是解码 DNG、读取 DNG/EXIF 和生成原始 camera metadata。场景亮度也可能来自：

- 视频容器或传感器 metadata；
- 外部测光表；
- sidecar 标定文件；
- 测试 harness 注入的逐帧 scene metadata。

如果把 EV 和场景标签写进 `rime-dng`，其他输入源必须反向依赖 DNG crate，形成错误的领域耦合。正确方式是 `rime-dng` 输出原始字段，调用方将其转换成 `rime-scene::SceneInput`。

### 3.3.2 为什么不是 `rime-isp`

`rime-isp` 应保持 operator-centric：method manifest、preprocess、shader、postprocess 和 module parameter packet。若在其中加入场景标签和物理换算，会导致：

- 非 ISP 使用者无法复用场景模型；
- AHD operator 变成 DNG/EXIF 感知模块；
- tone mapping、DRC 和其他模块各自复制 EV 计算；
- GPU/CPU 边界和纯函数测试变得混乱。

`rime-isp` 只消费已经解析好的 `SceneMeta`，并在 `preprocess` 中选择该 method 需要的轴。

### 3.3.3 `rime-core` 的边界

`rime-core` 可以放跨模块的通用 contract，例如 frame identity、revision、metadata provenance trait 或稳定的 metadata key 类型；不应放 `log2`、测光常数 `K=12.5`、CCT/EV 场景标签等摄影领域规则。这样 core 不会被某一种输入媒体或 ISP 产品模型污染。

### 3.3.4 推荐的 `rime-scene` API

首版 API 保持纯、可测试、与输入格式无关：

```rust
pub struct SceneInput {
    pub frame_index: u64,
    pub aperture_f_number: Option<f64>,
    pub exposure_time_seconds: Option<f64>,
    pub iso: Option<f64>,
    pub brightness_value: Option<f64>,
    pub exposure_bias_ev: Option<f64>,
    pub analog_gain: Option<f64>,
    pub digital_gain: Option<f64>,
    pub cct_kelvin: Option<f64>,
}

pub struct SceneMeta { /* typed values plus provenance */ }

pub fn ev100_capture(input: &SceneInput) -> Result<f64, SceneError>;
pub fn derive_scene_meta(input: &SceneInput, profile: &SceneProfile)
    -> Result<SceneMeta, SceneError>;
pub fn classify_scene(meta: &SceneMeta, labels: &SceneLabelSet)
    -> Option<SceneLabel>;
```

上面是边界示意，不是当前提交的代码 API。`rime-scene` 不返回 AHD 阈值，也不读取 IQ 表；它只产生可追溯的 scene facts 和派生 scene state。

## 4. SceneMeta 设计

### 4.1 推荐结构

场景元数据必须逐帧绑定，并携带来源和可信度：

```text
SceneMeta {
  frame_index

  capture {
    aperture_f_number
    exposure_time_seconds
    ev100_capture
  }

  scene_brightness {
    ev_apex
    luminance_cd_m2
    illuminance_lux
    source: measured | calibrated | estimated | unavailable
    confidence
    calibration_id
  }

  exposure {
    exposure_bias_ev
    metered_ev100
    exposure_deviation_ev
    source
  }

  gain {
    iso
    analog_gain
    digital_gain
    noise_state_id
  }

  color {
    cct_kelvin
    cct_source
  }
}
```

字段必须区分：

- 原始 metadata；
- 标准化派生量；
- 外部输入；
- 估计量；
- 缺失量。

禁止用 `0` 表示缺失，也禁止将缺失的 `ISO`、CCT 或亮度替换为“最近一帧值”。

### 4.2 DNG 来源优先级

对于场景亮度，采用以下优先级：

1. DNG/EXIF 明确提供的 `BrightnessValue` 或等价的相机测光数据；
2. 与当前 frame 精确绑定的外部测光/标定输入；
3. 由相机 profile、测光模型和 DNG 曝光信息得到的估计值；
4. 无法得到时标记 `unavailable`。

第 3 项不能标记为 `measured`。仅凭光圈、快门和 ISO，无法在不引入反射率、测光常数、曝光补偿和相机校准假设的情况下证明场景物理亮度。

对于当前仓库已有的 DNG 字段：

```text
exif_exposure_time
exif_f_number
exif_iso_speed
```

首先计算并保留：

```text
EV100_capture = log2(f_number^2 / exposure_time_seconds)
```

该字段用于曝光组合诊断和缺少更好场景输入时的显式 `estimated` 路径，不得直接命名为 `scene_luminance`。

后续 DNG metadata contract 应显式解析：

```text
BrightnessValue       EXIF 0x9203
ExposureBiasValue     EXIF 0x9204
```

`BrightnessValue` 表示相机测得的场景亮度 APEX 信息；`ExposureBiasValue` 表示曝光补偿，属于曝光偏差轴。二者必须分别进入 `SceneMeta`。

### 4.3 曝光偏差语义

如果相机提供测光 EV 和实际曝光 EV，可以定义：

```text
exposure_deviation_ev = actual_capture_ev100 - metered_scene_ev100
```

如果只有 `ExposureBiasValue`，则保留相机报告值：

```text
exposure_deviation_ev = exposure_bias_value
source = exif_exposure_bias
```

不能把两者无标注地混成同一个字段。参数包必须保存来源，以便复现和调试。

正值通常表示相机相对于测光基准增加曝光，负值表示减少曝光。这个量用于 tone mapping 时，表达“成像相对于场景目标的偏离”；它不是场景变亮或变暗。

### 4.4 ISO/gain 语义

ISO 是相机链路的增益状态，而不是场景亮度。它可能对应：

- sensor analog gain；
- ISP digital gain；
- 两者组合；
- 相机厂商报告的等效 ISO。

因此必须尽量保留：

```text
iso
analog_gain
digital_gain
noise_state_id
```

如果只有 ISO，则使用 `iso` 轴；如果相机 profile 提供实际 analog/digital gain，优先用 gain 派生 `noise_state_id`，同时保留 ISO 作为可解释的用户轴。

## 5. 场景标签

### 5.1 标签定位

场景标签是语义分类和调校样本组织方式，不是连续查表的替代物。连续参数必须按物理数值插值；标签边界不能导致 AHD 参数跳变。

标签用途：

- UI 中显示当前场景；
- IQ 调校样本分组；
- 快速选择调校 preset；
- 生成调校覆盖率和质量报告；
- 给 tone mapping 选择高层策略。

### 5.2 初始标签示例

以下范围是初始工程标签，不是对所有相机和测光环境的普适真值：

| 标签 | `scene_brightness_ev` 范围 | 典型场景 |
| --- | ---: | --- |
| `starlight_moonlight` | `-6 .. -2` | 星空、弱月光 |
| `night_street` | `-2 .. 2` | 夜间街道、低照度室外 |
| `dim_indoor` | `2 .. 5` | 暗室、酒吧、弱室内光 |
| `indoor` | `5 .. 8` | 普通室内、家庭照明 |
| `overcast_daylight` | `8 .. 11` | 阴天、窗边室内 |
| `daylight` | `11 .. 14` | 普通日光 |
| `bright_sun` | `14 .. 16` | 晴天直射阳光 |
| `extreme_brightness` | `16 .. 20` | 高反射、高照度场景 |

标签配置必须带版本和 profile，允许相机/测光系统针对实际标定调整范围。标签只是 metadata，不得成为 `if label == ...` 的 shader 分支。

## 6. AHD IQ 表

### 6.1 资产边界

推荐目录：

```text
crates/rime-isp/src/vbe/dem/
  dem04.rs
  dem04.wgsl
  dem04_preprocess.rs
  dem04_postprocess.rs
  dem04_iq_schema.yaml
  dem04_iq_default.yaml
```

运行时可将静态资产编译进 manifest，也可由受信任的 profile loader 载入版本化 YAML。无论载入方式如何，schema、默认表和 method 必须属于 AHD method `04` 的同一资产边界。

AHD IQ 表不能复制：

- DNG camera calibration matrix；
- noise profile 的原始标定资产；
- 已经派生完成的某一帧 module parameter；
- 其他模块的 IQ 表。

### 6.2 首版表结构

首版采用显式二维 Cartesian grid：

```yaml
schema_version: 1
module: vbe.dem
module_address: vbe.dem
method: "04"
style: default
parameter_schema_revision: dem04-v1

axes:
  - id: scene_brightness_ev
    source: scene_meta.scene_brightness.ev_apex
    unit: EV100_scene
    domain: [-6.0, 20.0]
    knots: [-4.0, 0.0, 4.0, 8.0, 12.0, 16.0]
    out_of_range: clamp
  - id: iso
    source: scene_meta.gain.iso
    unit: ISO
    knots: [100, 200, 400, 800, 1600, 3200, 6400]
    out_of_range: clamp

interpolation:
  type: multilinear
  coordinate_transform:
    scene_brightness_ev: linear
    iso: log2

effects:
  ahd_l_threshold:
    unit: lab_delta_l
    value_domain: normalized_shader_lab
    range: [0.0, 100.0]
    combine: multiply_log2
    values:
      dimensions: [scene_brightness_ev, iso]
      rows: [...]
  ahd_c_threshold_sq:
    unit: lab_delta_ab_squared
    value_domain: normalized_shader_lab
    range: [0.0, 10000.0]
    combine: multiply_log2
    values:
      dimensions: [scene_brightness_ev, iso]
      rows: [...]

modulation_curves: []
```

这里的 `rows` 不是无语义数组：schema 显式声明维度顺序、每个维度的 knots 和行列含义。实现校验必须验证矩阵形状与 knots 完整匹配。

ISO 使用 `log2` 坐标插值，因为 ISO/gain 的噪声变化更接近 stop/log 关系；场景亮度 EV 已经是对数单位，在 EV 域线性插值。

### 6.3 查找规则

给定 `SceneMeta` 和 `CameraMeta`：

1. 根据 `module_address`、method、style 和 revision 解析目标资产；
2. 校验所有必需轴是否存在、单位是否匹配；
3. 将场景亮度转换到表声明的 `EV100_scene`；
4. 将 ISO 转换到 `log2(ISO)` 坐标；
5. 执行二维 multilinear interpolation；
6. 按 parameter 的 `combine` 应用调制曲线；
7. 执行字段范围检查；
8. 按 AHD parameter schema 的 Rime.Q profile 量化；
9. 生成带来源 revision 的 `ModuleParameterPacket`。

默认边界策略为 `clamp`，禁止未声明的 extrapolation。缺少轴、单位错误、method 不匹配、表形状错误或调制曲线非法时必须拒绝加载。

### 6.4 参数值与效果值分离

IQ 表保存的是效果参数，不是最终 GPU 寄存器值：

```text
IQ effect value
  -> camera noise profile / sensor calibration
  -> scene meta
  -> modulation
  -> range validation
  -> Rime.Q quantization
  -> final ahd threshold uniform
```

同一套 IQ 表在不同 camera profile 下，可能生成不同的最终模块参数。`camera_profile_id` 和 `calibration_revision` 必须写入参数包审计信息。

## 7. AHD 调制曲线

### 7.1 设计目标

基础二维表解决主要场景变化；调制曲线解决单个轴的细粒度变化，避免建立稠密高维表。

调制曲线必须：

- 绑定明确的 axis source、unit 和 parameter；
- 具有 knots、范围和 clamp 策略；
- 明确 additive 或 multiplicative 组合方式；
- 经过 schema 校验；
- 在 preprocess 中计算，不能由 shader 临时查表；
- 具有曲线 revision，并进入最终参数包。

### 7.2 首选组合方式：log2 gain 曲线

两个阈值都是非负阈值，首版使用 `multiply_log2`：

\[
p_{final}=p_{base}\cdot2^{m(x)}
\]

其中：

- `p_base` 是基础二维表插值结果；
- `m(x)` 是轴 `x` 上的调制曲线，通常在 `0` 附近表示“不调制”；
- `p_final` 是校验和量化前的模块参数。

这样可以保证：

- 参数不会因为负的 additive offset 变成非法值；
- 不同参数量级可以使用统一的 stop-like 曲线；
- `ahd_c_threshold_sq` 继续以平方阈值形式保存，不需要开平方。

若某参数未来需要绝对差值调节，schema 可以单独声明 `additive`，但不能隐式混用。

### 7.3 曲线资产示例

```yaml
modulation_curves:
  - id: ahd_l_vs_exposure_deviation
    parameter: ahd_l_threshold
    axis:
      source: scene_meta.exposure.exposure_deviation_ev
      unit: EV
      knots: [-2.0, -1.0, 0.0, 1.0, 2.0]
      out_of_range: clamp
    combine: multiply_log2
    values: [0.12, 0.05, 0.0, -0.03, -0.08]

  - id: ahd_c_vs_cct
    parameter: ahd_c_threshold_sq
    enabled: false
    axis:
      source: scene_meta.color.cct_kelvin
      unit: K
      knots: [2800, 3500, 4500, 5500, 6500, 7500]
      out_of_range: clamp
    coordinate_transform: reciprocal_mired
    combine: multiply_log2
    values: [0.08, 0.04, 0.0, -0.02, -0.04, -0.06]
```

首版只启用已经有可靠调校数据的曲线。示例中的数值是 schema 示例，不是 GH5S 的最终 IQ 数值；正式默认表必须由测试样本调校得到，不能把示例数值宣称为校准结果。

### 7.4 调制顺序

固定顺序：

```text
base surface lookup
  -> scene/gain modulation
  -> optional exposure-deviation modulation
  -> optional CCT modulation
  -> camera-profile/noise-profile combination
  -> range validation
  -> quantization
```

资产必须声明调制顺序和每条曲线的组合方式。不能让 UI 用户任意重排曲线，因为重排会改变结果且破坏可复现性。

如果未来调制数量增长到难以解释，应升级为显式多维 surface，而不是继续叠加无界曲线。

## 8. 风格、override 和 revision

AHD 使用 canonical module address：

```text
vbe.dem
```

Graph-level override 只替换该模块 method `04` 的完整 IQ 表资产：

```yaml
style: low_noise_detail
base: default
schema_version: 1
overrides:
  vbe.dem: iq/low-noise-detail/vbe-dem-method-04.yaml
```

解析规则：

1. manifest 绑定 `vbe.dem` method `04` 的默认表；
2. style 根据 canonical address 应用完整替换；
3. 未覆盖时继续使用 default；
4. 覆盖表必须完整匹配 module、method、schema、axis unit 和字段类型；
5. 任何无效 override 都导致加载失败，不静默回退。

每个 AHD `ModuleParameterPacket` 的审计信息至少包含：

```text
module_address
method
frame_index
scene_meta_frame_index
camera_profile_id
calibration_revision
iq_table_id
iq_table_revision
style_id
style_revision
modulation_revision
parameter_schema_revision
module_parameter_revision
axis values and source/provenance
final effect values
final quantized module values
```

参数默认在 frame boundary 原子生效。正在执行的 AHD 不得被 UI 调整或新 IQ revision 追写。

### 8.1 初始 IQ、调节 IQ 和运行时参数的三层保存模型

三者必须分开保存，不能把一帧查表后的结果反向写回 IQ 资产：

```text
1. module default IQ asset       模块初始 IQ，产品/版本资产
2. tuning profile YAML           用户调节后的全管线 profile
3. resolved parameter snapshot   某帧最终生效参数，运行审计资产
```

#### A. 模块初始 IQ

模块初始 IQ 是模块 method 资产的一部分，由 `rime-isp` 随版本发布，保持只读：

```text
crates/rime-isp/src/vbe/dem/
  dem04_iq_schema.yaml
  dem04_iq_default.yaml
```

它是 `vbe.dem` / method `04` 的 factory default source of truth。不能由用户 profile 覆盖文件本身，也不能在运行时就地修改。默认资产至少带有：

```text
iq_table_id
iq_table_revision
module_address
module_id
method
parameter_schema_revision
axes
effects
modulation_curves
```

为了支持恢复出厂设置，UI 可以导出一个 `factory-default` profile；但该文件是 materialized export，不是默认 IQ 的第二个事实来源。

#### B. 用户调节后的 IQ

用户调节结果保存为一个完整的 tuning profile YAML。一个 profile 覆盖整条 pipeline 的所有可调 module instance，而不是为每个模块单独保存一个文件。

profile 中每个模块都有一个 entry：

- `inherit`：使用模块默认 IQ；
- `override`：使用该 entry 内嵌的完整 module IQ table；
- `unsupported`：manifest 中存在该模块，但当前 method 没有 IQ schema，仅用于明确覆盖范围。

`override` 必须是完整表资产，不使用任意字段级 YAML patch。这样 base 表增加字段时，旧 profile 不会静默继承出不可预测的混合结果。

#### C. 帧级 resolved parameter snapshot

profile 保存的是效果参数表、曲线和 override，不保存某一个 frame 的最终阈值。最终值依赖：

```text
profile IQ
SceneMeta[frame]
CameraMeta
noise profile
lookup/interpolation/modulation
Rime.Q quantization
```

因此运行审计另行保存 `ResolvedParameterSnapshot`，至少记录 profile revision、frame index、SceneMeta provenance、最终 effect values 和量化后的 module values。该 snapshot 可以写入运行日志或导出包，不能被当作下一次 profile 输入。

### 8.2 Tuning profile YAML

推荐的顶层结构：

```yaml
kind: rime.tuning_profile
schema_version: 1

profile:
  id: gh5s-standard
  name: GH5S Standard
  description: factory-derived baseline with AHD tuning
  created_by: user
  profile_revision: 7

pipeline:
  graph_id: normal
  manifest_revision: normal-v1
  base_iq_set: factory-default

camera:
  profile_id: pana-gh5s
  calibration_revision: gh5s-cal-v1

modules:
  vfe.blc:
    module_id: blc
    method: "00"
    tuning: unsupported

  vbe.wbc:
    module_id: wbc
    method: "00"
    tuning: inherit
    base:
      iq_table_id: vbe.wbc.method-00.default
      iq_table_revision: 1

  vbe.dem:
    module_id: dem
    method: "04"
    tuning: override
    base:
      iq_table_id: vbe.dem.method-04.default
      iq_table_revision: 1
    table:
      schema_version: 1
      axes: [...]
      effects:
        ahd_l_threshold: {...}
        ahd_c_threshold_sq: {...}
      modulation_curves: [...]

  vpe.mctf[1]:
    module_id: mctf
    binding_group: mctf_1
    method: "00"
    tuning: unsupported

  vpe.mctf[2]:
    module_id: mctf
    binding_group: mctf_2
    method: "00"
    tuning: unsupported
```

profile 的 `modules` 必须覆盖当前 manifest 的全部可调 module binding。endpoint、raw source 和 encoder 不属于 tuning module，不进入该表。没有 IQ schema 的模块使用 `unsupported`，不能用 `inherit` 冒充已有默认表。

profile 顶层必须保存 `graph_id`、`manifest_revision`、camera profile 和 base IQ set。加载时这些值用于拒绝错误 graph、错误 camera calibration 或不兼容的 module method。

### 8.3 多实例和共享调用的保存规则

profile key 必须是 manifest 展开的 canonical module instance address，不能使用 UI label、React node id 或当前 canvas 坐标：

```text
单个 DEM：       vbe.dem
两个 DEM：        vbe.dem[0], vbe.dem[1]
单个 VFE 的 BLC： vfe.blc
多个 VFE 的 BLC： vfe[0].blc, vfe[1].blc
```

同一个算法模块在 graph 中有多个共享调用位置时，profile 保存一个 `binding_group` entry，所有调用位置解析到同一个 tuning entry。例如当前 MCTF 设计中，三个尺度的 MCTF(1) 共享：

```text
binding_group: mctf_1
canonical profile key: vpe.mctf[1]
applies_to:
  vpe_16_mctf_1
  vpe_4_mctf_1
  vpe_full_mctf_1
```

如果两个调用位置需要不同 IQ，它们必须拥有不同的 canonical address 或 binding group。不能让 profile 通过顺序、显示名称或数组位置猜测实例对应关系。

### 8.4 读取、解析和生效顺序

profile 的唯一解析顺序：

```text
load profile YAML
  -> parse schema
  -> validate graph/manifest/camera compatibility
  -> expand canonical module instances
  -> resolve module default IQ for every entry
  -> apply complete profile overrides
  -> validate method/axis/unit/range/shape
  -> expose active profile to graph
  -> preprocess per frame
  -> produce ModuleParameterPacket
```

对于单个 module：

```text
module default IQ
        |
        +-- profile tuning: inherit  -> keep default
        |
        `-- profile tuning: override -> replace with complete table
                                      |
                                      v
                          SceneMeta + CameraMeta lookup
                                      |
                                      v
                           frame-level module packet
```

不允许以下隐式层级：

```text
default -> style -> profile -> graph patch -> UI patch -> shader
```

推荐将用户可见的 tuning profile 作为一个完整的 graph IQ selection。profile 内通过 `base_iq_set` 表达继承哪个 factory set，通过 `modules[address]` 表达稀疏的完整表替换。UI 的未保存修改只能存在于内存 draft；点击 Apply 后生成新的 graph/profile revision，点击 Save 后才写入 YAML。

### 8.5 保存和加载行为

#### 加载

1. 启动时加载内置 `factory-default`；
2. 用户选择 profile YAML；
3. host/profile service 解析 YAML，不能由 UI 自己拼装最终参数；
4. 对照当前 graph manifest 做严格校验；
5. 成功后建立 `ActiveTuningProfile`；
6. 下一合法 frame boundary 原子生效；
7. 失败时保留当前 active profile，不得半加载、半替换或静默降级。

#### 保存

1. UI 从 active profile 建立 draft；
2. 用户修改任意模块曲线或表值；
3. draft 经过完整 profile schema 校验；
4. `Save` 写入新的 `profile_revision`，不修改 factory asset；
5. `Apply` 使新 revision 在 frame boundary 生效；
6. 运行日志记录 active profile id/revision 和每帧最终参数来源。

保存路径由 host application 管理，例如：

```text
user tuning profiles/
  gh5s-standard.yaml
  gh5s-low-noise.yaml
  gh5s-detail.yaml
```

profile id 必须在加载时校验，不以文件名作为唯一身份。另存为时生成新的 profile id，不能覆盖原 profile 的历史 revision。

### 8.6 Profile 与现有 GraphIqOverride 的关系

现有 `GraphIqOverride` 只描述 graph presentation 中的绑定关系：

```text
override id + module id
```

它不适合直接作为完整 YAML tuning asset。建议保持职责分离：

```text
TuningProfile YAML
  -> profile resolver
  -> GraphIqOverride binding/reference
  -> module IQ table
  -> preprocess
```

`GraphIqOverride` 应增加或关联稳定的 profile/table reference，但不在 graph presentation 中内嵌完整曲线值。这样 graph 只表达“哪个模块实例绑定哪个 profile entry”，完整的模块参数资产仍由 tuning profile 管理。

profile resolver 必须保证：

- profile entry 的 `module_id` 与 canonical address 一致；
- method 与当前 selected method 一致；
- override schema 与 module IQ schema 一致；
- 一个实例不能同时命中多个 profile entry；
- 缺失 entry、重复 address、错误 binding group 和未知 module 都报错；
- 不得把无效 profile entry 当作 `inherit`。

### 8.7 版本和迁移

profile 需要同时记录：

```text
profile schema version
profile revision
graph manifest revision
module method
module parameter schema revision
base IQ table id/revision
camera calibration id/revision
```

method、参数 schema 或 IQ schema 不兼容时，默认拒绝加载。未来若提供迁移工具，迁移必须生成新的 profile id/revision，并保留旧 YAML，不允许 loader 在后台自动改写用户文件。

## 9. 可视化调节界面


### 9.1 入口

现有 `apps/desktop/src/components/NodeInspector.tsx` 已经根据 method manifest 渲染 DEM 参数。选择：

```text
Node: DEM
Method: 04 · ahd
```

后，在现有 Parameters 区域下增加 `IQ Tuning` 分组。

其他 DEM method 继续显示普通参数，不显示 AHD 专属曲线编辑器。

### 9.2 UI 分区

#### A. 当前输入状态

只读显示当前 frame 的查表输入：

```text
Scene brightness EV      8.5 EV
Luminance                42 cd/m² · estimated
Illuminance              unavailable
Exposure EV100 capture   9.1 EV
Exposure deviation       +0.7 EV · EXIF
ISO                      800
Analog gain              unavailable
Digital gain             unavailable
CCT                      5200 K · DNG / AWB source
Scene label              overcast_daylight
```

每个字段显示来源标识：`measured`、`calibrated`、`estimated` 或 `unavailable`。UI 不得只显示一个无来源的数字。

#### B. IQ 资产选择

```text
IQ table:        default
Style:           standard
Lookup mode:     automatic / manual override
Profile:         camera profile id
Revision:        iq revision
```

`automatic` 根据当前 frame 查表；`manual override` 仅用于调校和诊断，并且必须记录为 graph override，不得覆盖默认资产。

#### C. 参数结果卡片

分别显示：

```text
ahd_l_threshold
  base lookup value
  modulation delta / gain
  final effect value
  final quantized value
  range / unit / Rime.Q profile

ahd_c_threshold_sq
  base lookup value
  modulation delta / gain
  final effect value
  final quantized value
  range / unit / Rime.Q profile
```

用户拖动曲线时，必须同时显示 base、modulated 和 final，避免把曲线值误认为 shader 最终值。

#### D. 曲线编辑器

曲线编辑器支持：

- 选择参数；
- 选择可用轴：scene brightness、ISO/gain、exposure deviation、CCT；
- 显示 axis unit 和坐标变换；
- 添加、删除、拖动控制点；
- 精确编辑数值；
- clamp 边界显示；
- reset 到 IQ 资产值；
- 单调性约束开关；
- 曲线平滑仅作为可选显示/插值模式，不能私自改变资产声明；
- 显示当前 frame 在曲线上的位置；
- 显示当前 frame 的 lookup result；
- 对照预览中应用前后结果。

建议默认采用分段线性曲线，因为它与 IQ 资产的 knots、可复现性和 libcamera PWL 类实现一致。Cubic/spline 只有在明确声明边界、单调性和 overshoot 策略后才能加入。

#### E. 预览和提交

```text
[Preview current frame]
[Apply to graph override]
[Save IQ asset]
[Reset]
```

预览流程：

```text
UI curve draft
  -> schema validation
  -> temporary graph IQ override
  -> new config/method revision
  -> frame-boundary preprocess
  -> GPU preview
```

UI 不直接写 GPU uniform。预览和导出必须走同一套 lookup/preprocess 逻辑，保证调校看到的结果与最终运行结果一致。

### 9.3 缺失数据的界面行为

| 状态 | UI 行为 |
| --- | --- |
| 场景亮度 measured/calibrated | 自动查表并显示有效结果 |
| 场景亮度 estimated | 允许查表，但显著标记 estimated；调校导出要求用户确认来源 |
| 场景亮度 unavailable，表需要该轴 | 自动 IQ 显示 unavailable，禁止静默使用最近值；可切换到显式 manual override |
| ISO unavailable，表需要 ISO | 查表失败并显示缺失轴；不使用默认 ISO 100 猜测 |
| 表 schema 错误 | 阻止加载，显示错误路径和字段 |
| 超出 knot 范围 | 显示 clamp 状态和边界 knot |
| UI 编辑值越界 | 阻止提交，不截断成合法值后静默保存 |

### 9.4 开源曲线库评估

当前桌面端是 React 19 + Vite + Tauri，现有依赖中没有通用 chart library。调制曲线的核心需求不是普通数据展示，而是固定 knot、拖拽控制点、键盘微调、单调性约束、clamp、log2/EV 坐标和实时预览。因此库的选择必须服从 IQ asset contract，不能让图表库决定参数语义。

#### 方案 A：D3 + 自定义 React/SVG 曲线编辑器（推荐）

- [D3](https://github.com/d3/d3) 使用 ISC license；官方能力覆盖 scales、axes、shapes、drag 和 zoom。
- 使用 `d3-scale` 映射 EV、ISO、CCT 等坐标，使用 `d3-shape` 绘制分段线性路径，使用 `d3-drag` 或原生 Pointer Events 拖动控制点。
- React 只负责受控状态；曲线编辑器负责把屏幕坐标转换为 domain 值，并在提交前调用 IQ schema validator。

这是最适合本项目的方案。D3 不会把 AHD 曲线强行建模成普通 chart dataset，便于实现：

- EV 线性轴和 ISO `log2` 轴；
- 非均匀 knots；
- 当前 frame marker；
- 控制点不可越过相邻 knot；
- monotonic constraint；
- `multiply_log2` 与 additive 两种不同曲线语义；
- base/modulated/final 三条叠加曲线；
- 未来二维 IQ surface 的横截面视图。

代价是需要自己编写约 200--400 行的交互组件和键盘/可访问性处理，但这部分代码是项目的 IQ contract 适配层，不是重复实现图表库。

#### 方案 B：Chart.js + `chartjs-plugin-dragdata`（适合快速 prototype）

- [Chart.js](https://github.com/chartjs/Chart.js) 提供成熟的 canvas chart；
- [chartjs-plugin-dragdata](https://github.com/artus9033/chartjs-plugin-dragdata) 为 MIT license，当前 README 声明支持 Chart.js 3/4，并提供拖拽 x/y、`onDragStart`、`onDrag`、`onDragEnd` 和 magnet 回调；
- 插件仓库也提供 React integration example。

优点：

- 最快得到可拖拽的折线图；
- tooltip、坐标轴、缩放和数据点交互已有实现；
- 可以用 `onDrag` 回调拒绝越界值，用 magnet 对齐到合法 knot。

缺点：

- 需要额外引入 Chart.js 和 React wrapper；
- chart dataset 语义与 IQ knots/曲线资产不是同一模型；
- 自定义 log2 坐标、固定 x knot、禁止移动 x、二维表横截面和多层 base/final 叠加需要较多适配；
- 自动注册行为在插件下一个 major 版本计划移除，集成时必须显式注册；
- 对一个参数编辑器而言依赖重量大于实际需求。

结论：可以用于 Phase 1 的交互原型，不建议作为最终 AHD IQ 编辑器的长期基础。

#### 方案 C：直接采用现成 Bézier editor（不推荐）

- [Motion BezierCurveEditor](https://motion.dev/docs/ai-kit-sdk-bezier-curve-editor) 有键盘和触摸交互，但文档明确该能力属于 Motion+ early access，不是适合本项目的开源依赖。
- [bestak/bezier-curve-editor](https://github.com/bestak/bezier-curve-editor) 是轻量 Bézier/line 编辑器，但仓库 API 当前没有声明 license，且活跃度和社区规模不足；在许可证明确前不能作为项目依赖。

此外，通用照片编辑器组件如 `react-image-editor` 面向完整图片编辑，不适合作为 IQ 曲线的基础控件：它拥有更大的产品语义和较弱的 IQ schema 约束，容易把“显示曲线”和“生成可复现参数资产”混在一起。

#### 最终选择

采用：

```text
React controlled component
  + SVG rendering
  + D3 scale/shape/drag utilities
  + project-owned IQ curve model and validator
```

不使用 `@xyflow/react` 实现曲线编辑。该依赖已经用于 graph canvas，但 graph node/edge editor 的数据模型与 IQ control-point curve 不同，强行复用会引入错误的状态和交互抽象。

曲线编辑器必须只编辑 domain-level curve draft，不能直接编辑最终 shader uniform。所有 draft 在应用前经过与 native preprocess 相同的 schema、范围、单调性和 revision 校验。

## 10. Tone mapping 与 AHD 的关系

场景亮度和曝光偏差首先为 tone mapping 提供 scene-referred 信息：

```text
scene_luminance / scene_illuminance
exposure_deviation_ev
        |
        v
scene-referred tone / dynamic-range policy
```

AHD 只消费自身需要的部分：

```text
scene brightness + ISO/noise state
        |
        v
AHD threshold IQ
```

这样可以让：

- tone mapping 根据场景物理亮度恢复或压缩影调；
- exposure deviation 表达相机成像与场景目标之间的偏差；
- AHD 根据 RAW 噪声和局部场景条件调整同质性判定；
- 后续模块独立选择轴，不被 DEM 的 schema 绑架。

跨模块共享只能通过 graph-level `SceneMeta` 或显式 parameter edge，不能让 AHD 读取 tone mapping 模块的私有状态。

## 11. 开源实现参考

这些项目用于参考架构和工程实践，不直接复制代码。任何代码复用都必须单独进行许可证审查。

### 11.1 Raspberry Pi libcamera

- [Raspberry Pi libcamera README](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/README.md)
- [Controller tuning loader](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/controller/controller.cpp)
- [AWB CCT curve implementation](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/controller/rpi/awb.cpp)

可借鉴：

- 版本化 tuning asset；
- 命名算法 block；
- 参数读取和 schema 错误在加载阶段失败；
- `prepare/process` 生命周期边界；
- CCT 使用显式 piecewise-linear curve；
- 算法状态与 tuning 配置分离。

libcamera 当前 controller 目录可通过仓库 API 查看；不能假定某个 sensor tuning 文件路径在所有分支都存在。

### 11.2 darktable

- [demosaic module](https://github.com/darktable-org/darktable/blob/master/src/iop/demosaic.c)

可借鉴：

- 参数的显式 min/max/default/description 元数据；
- method 列表与参数结构分离；
- GUI 控件按参数类型绑定；
- 预览、模块参数和实际执行边界清晰。

当前 darktable 的 demosaic method 集合并不是本项目 AHD method `04` 的直接参数来源，因此只借鉴参数和 UI 组织方式。

### 11.3 RawTherapee

- [demosaic algorithms](https://github.com/Beep6581/RawTherapee/blob/dev/rtengine/demosaic_algos.cc)

可借鉴：

- demosaic 算法实现边界；
- 数值保护和边界处理；
- 不同算法 method 的独立实现路径。

### 11.4 ExifTool

- [EXIF tag definitions](https://github.com/exiftool/exiftool/blob/master/lib/Image/ExifTool/Exif.pm)

可用于核对：

```text
BrightnessValue       0x9203
ExposureBiasValue     0x9204
ExposureTime          0x829A
FNumber               0x829D
ISOSpeedRatings       0x8827
```

### 11.5 EV 定义

- [Exposure value](https://en.wikipedia.org/wiki/Exposure_value)

用于标准 APEX EV 术语和曝光值关系。物理亮度/照度换算仍必须在项目中声明所用测光常数、假设和校准来源，不能只保存一个无来源的 EV 数。

## 12. 实现分阶段计划

### Phase 1：SceneMeta 与纯函数

- 增加 APEX EV 计算纯函数；
- 增加 `BrightnessValue`、`ExposureBiasValue` 的 metadata contract；
- 生成 `scene_brightness`、`exposure_deviation`、`gain/noise` 的 typed model；
- 实现来源、可信度、缺失和 frame binding；
- 为标准 EV→亮度/照度换算增加独立测试。

### Phase 2：IQ schema 和 loader

- 定义 method `04` 的 IQ schema；
- 实现显式轴、单位、knots、二维 grid 形状校验；
- 实现 EV 线性坐标和 ISO log2 坐标；
- 实现 clamp、缺轴拒绝、schema revision；
- 加载默认 AHD IQ 表和 graph override。

### Phase 3：AHD preprocess

- 将 SceneMeta、CameraMeta 和 IQ effect params 传入 `dem04_preprocess`；
- 在 preprocess 完成 lookup、调制、范围检查和 Rime.Q 量化；
- 生成冻结的 AHD ModuleParameterPacket；
- 让 compute 只读取 packet，不读取全局参数。

### Phase 4：调制曲线

- 实现 `multiply_log2`；
- 支持 exposure deviation 和 CCT 的可选曲线；
- 固定曲线组合顺序；
- 增加曲线形状、范围、重复 knot 和非法单位校验。

### Phase 5：桌面端 UI

- 扩展 NodeInspector 的 AHD method 专属面板；
- 增加 SceneMeta 来源和查表状态显示；
- 增加参数结果卡片；
- 增加分段线性曲线编辑器；
- 增加 preview/apply/reset/export；
- 让 graph override 和 revision 进入现有 runtime command/authority contract。

### Phase 6：调校数据和 profile

- 使用 GH5S DNG 样本建立 scene brightness、ISO 和 AHD artifact 标注；
- 确认 `ahd_l_threshold` 与 `ahd_c_threshold_sq` 的有效范围；
- 评估 exposure deviation 是否需要进入 AHD 调制；
- 再增加 CCT、analog/digital gain 和 noise profile 轴。

## 13. 验证要求

### 13.1 公式和元数据

- `EV100_capture` 对不同光圈/快门组合结果正确；
- ISO 不改变 `EV100_capture`，只影响独立 gain/noise state；
- `BrightnessValue` 不被误读为 aperture/shutter EV；
- `ExposureBiasValue` 不被并入 scene brightness；
- 缺失或非法 rational 不产生伪造的零值；
- 物理亮度换算标记 `measured`、`calibrated` 或 `estimated`。

### 13.2 IQ lookup

- 轴来源、单位和 method 必须匹配；
- EV 在 knots 之间执行确定性线性插值；
- ISO 在 log2 坐标中插值；
- 越界只按声明执行 clamp；
- 缺少必需轴直接失败；
- 重复 knot、非法范围、矩阵形状错误和未知字段直接失败；
- 同一 snapshot/revision 生成 bit-reproducible effect values 和 packet。

### 13.3 AHD 参数

- `ahd_l_threshold` 的单位和值域明确；
- `ahd_c_threshold_sq` 始终以平方色度差形式保存；
- 调制曲线不产生负阈值或越界值；
- 最终量化值与 shader 使用的 uniform 值一致；
- compute 不会绕过 packet 重新查 IQ 表。

### 13.4 UI 行为

- AHD method `04` 显示两个参数和 IQ 面板；
- 其他 method 不显示不相关的 AHD 曲线；
- 当前 frame marker 随 frame 变化；
- estimated/unavailable 状态可见；
- 缺轴时没有静默最近值回退；
- 曲线编辑结果只在合法 frame boundary 生效；
- preview 与实际运行采用同一 lookup/preprocess 路径；
- apply/reset/export 的 revision 和 override 可追踪。

## 14. 设计决策总结

1. 场景亮度是拍摄场景的物理/scene-referred 量，不是相机曝光组合的别名。
2. `EV100_capture = log2(N²/t)` 保留为曝光组合诊断量。
3. 标准 EV→luminance/illuminance 表作为亮度语义换算层；`K=12.5` 只用于声明的反射式亮度模型，照度使用独立常数。
4. 曝光补偿/曝光偏差独立于场景亮度，主要为 tone mapping 提供成像偏离信息。
5. ISO、analog gain、digital gain 独立表示 RAW 放大和含噪状态，不能折算成场景变暗。
6. AHD 首版采用 `scene brightness × ISO/noise` 二维基础表。
7. exposure deviation 和 CCT 通过可选调制曲线接入，不能隐式成为全局轴。
8. AHD 的两个阈值在 preprocess 中查表、调制、校验和量化；WGSL 只消费冻结 packet。
9. 场景标签用于语义、UI 和样本组织，不用于产生参数跳变。
10. 所有缺失轴、错误单位、非法表和不匹配 override 必须显式失败，禁止静默回退。

## 15. 参考文档

- `docs/rime-frameforge-top-architecture-design.md`，尤其是第 8 节 IQ tuning architecture；
- [Exposure value — Wikipedia](https://en.wikipedia.org/wiki/Exposure_value)；
- [Raspberry Pi libcamera tuning README](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/README.md)；
- [Raspberry Pi libcamera controller](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/controller/controller.cpp)；
- [Raspberry Pi libcamera AWB CCT curve](https://github.com/raspberrypi/libcamera/blob/main/src/ipa/rpi/controller/rpi/awb.cpp)；
- [darktable demosaic](https://github.com/darktable-org/darktable/blob/master/src/iop/demosaic.c)；
- [RawTherapee demosaic algorithms](https://github.com/Beep6581/RawTherapee/blob/dev/rtengine/demosaic_algos.cc)；
- [ExifTool EXIF definitions](https://github.com/exiftool/exiftool/blob/master/lib/Image/ExifTool/Exif.pm)。
- [D3](https://github.com/d3/d3)；
- [D3 official documentation](https://d3js.org/)；
- [chartjs-plugin-dragdata](https://github.com/artus9033/chartjs-plugin-dragdata)；
- [SVG-Edit React](https://github.com/SVG-Edit/svg-edit-react)（通用 SVG 编辑器参考，不作为 IQ 曲线依赖）。
