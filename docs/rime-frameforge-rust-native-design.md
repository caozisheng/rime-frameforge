# Rime FrameForge Rust Native GPU 架构设计

## 1. 目标

将 DNG 解码、ISP graph 执行和视频编码逐步迁移到 Rust native pipeline，同时保留 Tauri + React 作为桌面 UI 和控制面，并提供 headless CLI，支持：

```text
单帧 DNG      → PNG
DNG sequence → FFmpeg 压缩视频
```

CLI 与桌面应用共享同一套 graph IR、参数 contract、DNG frame contract 和 native execution pipeline，使未来 agent 可以通过命令行稳定地调图、批处理和导出。

非目标：本阶段不迁移到 GPUI；不把 UI 重写为 Rust；不把所有 DNG 一次加载到内存；不让 FFmpeg stdin 承担 GPU texture 传输。

## 2. 核心判断

Rust native GPU 不等于 GPUI。

- **Tauri**：窗口、菜单、React UI、Inspector、控制命令。
- **Rust native GPU**：DNG decode、GPU graph、buffer/texture 生命周期、调度。
- **FFmpeg/NVENC**：硬件编码和容器 mux。
- **CLI**：headless file-to-file orchestration。

Tauri 可以继续保留。只有当产品目标变成完全脱离 WebView 的 Rust UI 时，才需要单独评估 GPUI。

## 3. 目标分层

```text
rime-core
├── Graph IR
├── PipelineManifest
├── graph validation/topology
├── parameter revisions
└── frame/execution contracts

rime-dng
└── DNG parsing and one-frame decode

rime-native-gpu
├── native GPU device/context
├── graph executor
├── fused/segmented pass planner
├── bounded frame ring
└── resource ownership

rime-encode
├── NVENC hardware encoder adapter
├── FFmpeg codec/mux adapter
└── compressed packet output

rime-cli
└── headless file-to-file commands

apps/desktop/src-tauri
├── Tauri command adapter
├── native pipeline session
└── UI event/progress bridge

React/Web UI
└── controls, graph visualization, Inspector, preview, logs
```

## 4. 数据流

### 4.1 单帧 DNG → PNG

```text
input.dng
   ↓
rime-dng decode one frame
   ↓
native GPU ISP graph
   ↓
GPU→CPU readback of final RGBA/RGB surface
   ↓
PNG encoder
   ↓
output.png
```

单帧 PNG 允许一次最终 readback，因为 PNG 是 CPU 输出格式。RAW 不经过 Tauri IPC。

### 4.2 DNG sequence → 压缩视频

```text
DNG sequence
   ↓
Rust bounded decoder ring
   ↓
native GPU ISP graph
   ↓
GPU NV12/P010 surface
   ↓
NVENC hardware encoder
   ↓
compressed packets
   ↓
FFmpeg muxer
   ↓
output video
```

目标路径中不把完整分辨率视频帧拷贝回 CPU，也不通过 FFmpeg stdin 传 rawvideo。CPU 只处理：

- DNG 文件读取和解码；
- 参数/metadata；
- NVENC 压缩 packet；
- FFmpeg mux；
- progress/error。

## 5. GPU 后端策略

### 5.1 第一阶段：native GPU + readback

先实现 Rust native GPU graph，最终结果允许 readback。用途：

- 验证 graph 结果与现有 WebGPU 输出一致；
- 验证参数和 frame contract；
- 实现 CLI；
- 输出 PNG；
- 通过 FFmpeg stdin 或 pipe 做视频功能验证。

该阶段不承诺 zero-copy encoder。

### 5.2 第二阶段：D3D11/D3D12 + NVENC interop

Windows + NVIDIA 首选：

```text
native GPU graph
   ↓ D3D11/D3D12 texture
NVENC input surface
```

`wgpu` 公共抽象不保证可导出 NVENC 所需的 native texture handle。zero-copy 需要：

- D3D11/D3D12 native resource ownership；
- texture handle/import contract；
- queue/resource synchronization；
- NVENC registered resource 生命周期；
- device loss recovery。

因此 native GPU 模块必须把 backend 分为：

```text
NativeGpuBackend::WgpuReadback
NativeGpuBackend::D3dNvencZeroCopy
```

不能把 native handle 假装成跨平台 `wgpu::Texture` contract。

### 5.3 CUDA 作为可选后端

CUDA graph + NVENC 互操作更直接，但会：

- 绑定 NVIDIA/CUDA toolkit/driver；
- 失去 AMD/Intel 支持；
- 需要迁移 WGSL 到 CUDA kernel；
- 增加部署和 CI 复杂度。

除非 D3D interop 证明不可行，否则不作为第一 native backend。

## 6. ISP graph 执行模型

### 6.1 Graph IR 与执行计划

`rime-core` 继续拥有平台无关 graph IR：

- node/port/domain/format；
- topology；
- method；
- bypass/disabled 状态；
- preview output；
- frame delay；
- parameter revision。

native GPU backend 将 IR 编译成：

```text
FusedPlan
├── pull-fused local chain
└── materialization boundaries for non-local nodes
```

### 6.2 Fused 与 segmented

局部采样链使用 pull fusion：

```text
output sample
  → RGB2YUV
  → Gamma
  → CCM
  → DEM/BLC/WBC
  → RAW
```

bypass 节点编译期消除，不生成 identity pass。

复杂或非局部节点保留 pass boundary：

```text
pre-fused
  → materialized DEM/statistics/FFT/temporal pass
  → post-fused
```

pass boundary 由 node capability 决定，不由“一个 graph node 一个 pass”决定。

### 6.3 同步原则

同一 command buffer 内的 graph 依赖由 GPU queue 顺序保证，不使用节点级 CPU await。

正常帧：

```text
encode all graph passes
→ one queue submit
→ one frame-level fence when ownership/readback requires it
```

只有这些情况允许等待：

- 最终 readback；
- NVENC surface ownership transfer；
- 资源销毁/重建；
- device lost/reset；
- query/readback 结果。

## 7. 有界流水线

无论 CLI 还是 Tauri，sequence 使用固定容量 ring：

```text
DecodeSlot[0..N-1]
GpuSlot[0..N-1]
EncodeSlot[0..N-1]
```

默认 `N = 2`，必要时配置为 3。禁止按 sequence 长度增长。

状态：

```text
Empty → Decoding → Decoded → GpuSubmitted → Encoded → Reusable
```

backpressure：

- decode 快于 GPU：decoder 等待空 slot；
- GPU 快于 decode：GPU 等待 decoded slot；
- encoder 慢于 GPU：GPU 等待 encode slot；
- queue 满时不继续读取文件。

Pause/cancel 时：

- 停止提交新 frame；
- 清理未提交 decoded slot；
- 保留已提交 GPU frame；
- sequence generation 递增，拒绝旧结果。

## 8. DNG decoder contract

`rime-dng` 每次只返回一个 `DecodedRawFrame`：

```rust
DecodedRawFrame {
    frame_index,
    layout,
    metadata,
    samples,
}
```

decoder 层不负责 sequence buffering。sequence scheduler 只持有固定数量 frame slot。

对于无压缩 DJI X5S DNG：

- 读取 payload 接近固定 offset read；
- 不应重复 decode preview/semantic mask/额外 sub-images；
- metadata 解析只读取 graph 所需字段；
- 原始 sample buffer 只保留一份；
- 不通过 JSON number array 传输像素。

未来可增加 `RawDecodeMode::Pipeline`，跳过与 graph 无关的 preview/sub-image 解析；但必须保持 `RawDecodeMode::Inspect` 的完整 metadata 能力。

## 9. FFmpeg/NVENC contract

### 9.1 不固定 codec

CLI 不强制默认 H.264/HEVC。agent 或用户指定：

```text
codec
preset
profile
bitrate / CQ / CRF-equivalent
GOP
B-frames
pixel format
container
```

CLI 负责校验参数并映射到 encoder backend，不接受任意字符串直接拼接 shell command。

### 9.2 Encoder interface

```rust
trait VideoEncoder {
    fn configure(&mut self, config: EncoderConfig) -> Result<()>;
    fn submit_gpu_frame(&mut self, frame: GpuVideoFrame) -> Result<()>;
    fn receive_packet(&mut self) -> Result<Option<EncodedPacket>>;
    fn finish(&mut self) -> Result<()>;
}
```

`GpuVideoFrame` 必须包含：

- backend/device identity；
- native resource handle；
- width/height；
- pixel format NV12/P010；
- color metadata；
- frame index/timestamp；
- ownership state。

FFmpeg mux 只接收 encoded packets，不接收 GPU texture。

### 9.3 Backend fallback

```text
D3D/NVENC zero-copy available
  → use hardware encoder
otherwise
  → native GPU readback + FFmpeg CPU input
```

fallback 必须明确记录：

```text
encoder_backend = nvenc_zero_copy | cpu_readback
```

不得静默宣称 zero-copy。

## 10. CLI 设计

二进制建议命名：

```text
rime-frameforge
```

子命令：

```text
rime-frameforge inspect <input.dng>
rime-frameforge render <input.dng> --output output.png
rime-frameforge render-sequence <input.dng> --output output.mp4 ...
rime-frameforge graph validate --graph normal
rime-frameforge graph show
```

### 10.1 单帧

```text
rime-frameforge render input.dng --output output.png \
  --dem-method 00 \
  --graph-config config.json
```

### 10.2 sequence

```text
rime-frameforge render-sequence input.dng --output output.mp4 \
  --codec hevc \
  --encoder nvenc \
  --preset p5 \
  --bitrate 20M \
  --fps 24 \
  --graph-config config.json
```

选中任意 sequence 成员后，CLI 与桌面行为一致：扫描直接父目录、过滤 DNG、自然排序。

### 10.3 stdout 与 JSON

为 agent 提供：

```text
--json
--quiet
--progress jsonl
--dry-run
```

示例 progress event：

```json
{"event":"frame_started","index":12,"total":120}
{"event":"frame_completed","index":12,"milliseconds":184}
{"event":"completed","frames":120,"output":"output.mp4"}
```

stdout 只输出机器可读事件；日志写 stderr。

## 11. Agent 调图接口

Agent 不直接操作 GPU 或拼接 FFmpeg shell 字符串，只提交声明式 graph config：

```json
{
  "graph": "normal",
  "input": "sequence.dng",
  "output": "output.mp4",
  "nodes": {
    "dem": { "method": "00" },
    "blc": { "rimeQ": { "enabled": true, "profile": "s0.14", "clipType": "truncate" } },
    "wbc": { "rimeQ": { "enabled": true, "profile": "s0.12", "clipType": "round" } }
  },
  "encoder": {
    "backend": "nvenc",
    "codec": "hevc",
    "preset": "p5",
    "bitrate": "20M"
  }
}
```

执行前必须：

1. 校验 graph/node/method/parameter；
2. 校验输入 sequence；
3. 校验 GPU backend 和 encoder capability；
4. 输出 resolved config；
5. 再执行。

建议提供：

```text
rime-frameforge render ... --print-resolved-config
```

这样 agent 可读回实际生效参数，避免“请求参数”和“硬件实际配置”不一致。

## 12. Tauri 集成

Tauri 只调用 native pipeline session：

```text
Tauri command
  → Rust pipeline controller
  → progress/metadata/preview events
```

不再通过 Tauri IPC 发送完整 RAW frame。

Tauri 事件只传：

- frame index；
- sequence count；
- metadata；
- progress；
- timing；
- low-resolution preview 或 encoded preview；
- error。

React UI 继续负责：

- graph canvas；
- inspector；
- transport controls；
- preview surface；
- logs。

## 13. 迁移阶段

### Phase 1：Rust native graph + readback

- 抽取平台无关 graph executor contract；
- Rust 实现 native GPU backend；
- 单帧 DNG → PNG；
- sequence → CPU/readback → FFmpeg 视频；
- 与当前 WebGPU 输出做 golden 对比。

### Phase 2：headless CLI

- 增加 `rime-frameforge` binary；
- 支持 render/inspect/render-sequence；
- JSON progress；
- agent config validation；
- 固定容量 sequence ring。

### Phase 3：Tauri 切换到 native pipeline

- Tauri 后端启动 pipeline session；
- UI 接收 metadata/progress/preview；
- 移除完整 RAW IPC；
- 保留 WebGPU 路径作为开发对照或 fallback。

### Phase 4：NVENC zero-copy

- D3D11/D3D12 resource export/import；
- NVENC registration；
- frame fence/ownership；
- NV12/P010 output；
- fallback 和 capability reporting。

### Phase 5：扩展 graph

- 20+ 节点逐步接入；
- 融合 capability；
- non-local pass boundary；
- temporal state；
- VPE pyramid；
- per-node timing 和 GPU profiling。

## 14. 验证标准

### 正确性

- native graph 与 WebGPU 输出在允许误差内一致；
- CFA 四种排列正确；
- DEM 00–04 正确；
- Rime.Q profiles/ClipType 一致；
- sequence 帧号、时间戳不乱序；
- 单帧 PNG 可读取；
- sequence 视频可由 FFmpeg 正确 mux/playback。

### 内存

- 内存占用不随 sequence 长度线性增长；
- ring 容量固定；
- cancel/pause 后 slot 可回收；
- GPU device loss 不泄漏资源。

### 性能

分别记录：

```text
DNG read
DNG metadata/decode
CPU→native GPU upload
GPU graph
GPU→CPU readback（fallback）
NVENC submit
FFmpeg mux
end-to-end frame
```

目标不是单独优化某一层，而是确认稳态：

```text
throughput = 1 / max(stage durations)
```

### Agent 可操作性

- dry-run 不产生输出文件；
- 非法参数返回稳定错误码；
- JSON progress 可逐行解析；
- resolved config 可复现；
- 同一 config 在 CLI/Tauri 中得到相同 graph 行为。

## 15. 主要风险

1. `wgpu` native handle interop 不是稳定跨平台 API；必须隔离 backend。
2. NVENC texture ownership 与 GPU queue synchronization 需要明确 fence contract。
3. FFmpeg arbitrary options 不能直接暴露为不校验 shell 字符串。
4. 复杂 DEM、统计、FFT、temporal feedback 不能强制 pull fuse。
5. Windows NVIDIA 路径优先，但 CLI 仍需 capability-aware fallback。
6. Rust native graph 与当前 WebGPU graph 双实现期间必须持续 golden 对比。
7. 单帧 PNG 的 readback 可以接受，视频稳定路径不应依赖 readback。

## 16. 最终决策

采用：

```text
Tauri + React：UI/control plane
Rust：DNG decode + graph orchestration
native GPU backend：graph execution
FFmpeg/NVENC：GPU video encoding + mux
CLI：headless file-to-file and agent entry point
```

不采用：

```text
GPUI migration as a prerequisite
FFmpeg stdin for GPU texture transport
whole-sequence preload
unvalidated arbitrary FFmpeg shell arguments
```

第一实现顺序：

```text
Rust native GPU + readback
→ CLI single DNG → PNG
→ CLI DNG sequence → FFmpeg video
→ Tauri native session
→ D3D/NVENC zero-copy
```
