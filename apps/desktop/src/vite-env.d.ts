/// <reference types="vite/client" />
/// <reference types="@webgpu/types" />

declare module '*.wgsl?raw' {
  const source: string;
  export default source;
}

declare module '*.raw?url' {
  const url: string;
  export default url;
}
