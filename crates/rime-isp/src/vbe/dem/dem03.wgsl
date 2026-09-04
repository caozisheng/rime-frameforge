struct DemosaicParams {
  cfa_pattern: vec4<u32>,
  thresholds: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: DemosaicParams;
@group(0) @binding(1) var input_tex: texture_2d<f32>;
@group(0) @binding(2) var output_tex: texture_storage_2d<rgba32float, write>;

fn sample(p: vec2<i32>, extent: vec2<u32>) -> f32 { return textureLoad(input_tex, clamp(p, vec2<i32>(0), vec2<i32>(extent) - vec2<i32>(1)), 0).r; }
fn cfa(p: vec2<i32>, extent: vec2<u32>) -> u32 { let q=clamp(p,vec2<i32>(0),vec2<i32>(extent)-vec2<i32>(1)); let phase=vec2<u32>(u32(q.x)&1u,u32(q.y)&1u); return params.cfa_pattern[phase.y*2u+phase.x]; }
fn green_at(p: vec2<i32>, e: vec2<u32>) -> f32 {
  let position=clamp(p,vec2<i32>(0),vec2<i32>(e)-vec2<i32>(1));
  if(cfa(position,e)==1u){return sample(position,e);} let c=sample(position,e);
  let n1=sample(position+vec2<i32>(0,-1),e);let n2=sample(position+vec2<i32>(0,-2),e);let s1=sample(position+vec2<i32>(0,1),e);let s2=sample(position+vec2<i32>(0,2),e);
  let e1=sample(position+vec2<i32>(1,0),e);let e2=sample(position+vec2<i32>(2,0),e);let w1=sample(position+vec2<i32>(-1,0),e);let w2=sample(position+vec2<i32>(-2,0),e);
  let ne1=sample(position+vec2<i32>(1,-1),e);let ne2=sample(position+vec2<i32>(2,-2),e);let nw1=sample(position+vec2<i32>(-1,-1),e);let nw2=sample(position+vec2<i32>(-2,-2),e);
  let se1=sample(position+vec2<i32>(1,1),e);let se2=sample(position+vec2<i32>(2,2),e);let sw1=sample(position+vec2<i32>(-1,1),e);let sw2=sample(position+vec2<i32>(-2,2),e);
  let g0=abs(n2-c)+abs(n1-s1)+abs(nw1-sw1);let g1=abs(ne2-c)+abs(ne1-sw1);let g2=abs(e2-c)+abs(e1-w1)+abs(ne1-nw1);let g3=abs(se2-c)+abs(se1-nw1);
  let g4=abs(s2-c)+abs(s1-n1)+abs(se1-ne1);let g5=abs(sw2-c)+abs(sw1-ne1);let g6=abs(w2-c)+abs(w1-e1)+abs(sw1-se1);let g7=abs(nw2-c)+abs(nw1-se1);
  let minimum=min(min(min(g0,g1),min(g2,g3)),min(min(g4,g5),min(g6,g7)));let threshold=minimum*params.thresholds.x;
  let v0=n1+0.5*(c-n2);let v1=0.5*(n1+e1)+0.25*(2.0*c-n2-e2);let v2=e1+0.5*(c-e2);let v3=0.5*(s1+e1)+0.25*(2.0*c-s2-e2);
  let v4=s1+0.5*(c-s2);let v5=0.5*(s1+w1)+0.25*(2.0*c-s2-w2);let v6=w1+0.5*(c-w2);let v7=0.5*(n1+w1)+0.25*(2.0*c-n2-w2);
  var sum=0.0;var count=0.0;if(g0<=threshold){sum+=v0;count+=1.0;}if(g1<=threshold){sum+=v1;count+=1.0;}if(g2<=threshold){sum+=v2;count+=1.0;}if(g3<=threshold){sum+=v3;count+=1.0;}
  if(g4<=threshold){sum+=v4;count+=1.0;}if(g5<=threshold){sum+=v5;count+=1.0;}if(g6<=threshold){sum+=v6;count+=1.0;}if(g7<=threshold){sum+=v7;count+=1.0;}
  if(count>0.0){return sum/count;}return 0.125*(v0+v1+v2+v3+v4+v5+v6+v7);
}
fn cardinal_difference(p:vec2<i32>,e:vec2<u32>,horizontal:bool)->f32{let position=clamp(p,vec2<i32>(0),vec2<i32>(e)-vec2<i32>(1));let a=select(position+vec2<i32>(0,-1),position+vec2<i32>(-1,0),horizontal);let b=select(position+vec2<i32>(0,1),position+vec2<i32>(1,0),horizontal);return 0.5*(sample(a,e)-green_at(a,e)+sample(b,e)-green_at(b,e));}
fn diagonal_difference(p:vec2<i32>,e:vec2<u32>,target_channel:u32)->f32{let position=clamp(p,vec2<i32>(0),vec2<i32>(e)-vec2<i32>(1));var sum=0.0;var count=0.0;for(var dy:i32=-1;dy<=1;dy+=2){for(var dx:i32=-1;dx<=1;dx+=2){let q=position+vec2<i32>(dx,dy);if(cfa(q,e)==target_channel){sum+=sample(q,e)-green_at(q,e);count+=1.0;}}}return sum/max(count,1.0);}
fn reconstruct(p:vec2<i32>,e:vec2<u32>)->vec3<f32>{let position=clamp(p,vec2<i32>(0),vec2<i32>(e)-vec2<i32>(1));let channel=cfa(position,e);let green=green_at(position,e);var rgb=vec3<f32>(green);if(channel==0u){rgb.r=sample(position,e);rgb.b=green+diagonal_difference(position,e,2u);}else if(channel==2u){rgb.r=green+diagonal_difference(position,e,0u);rgb.b=sample(position,e);}else{let row=cfa(vec2<i32>(position.x^1,position.y),e);if(row==0u){rgb.r=green+cardinal_difference(position,e,true);rgb.b=green+cardinal_difference(position,e,false);}else{rgb.b=green+cardinal_difference(position,e,true);rgb.r=green+cardinal_difference(position,e,false);}}return rgb;}
@compute @workgroup_size(8,8)
fn demosaic_vng_main(@builtin(global_invocation_id) gid:vec3<u32>){let extent=textureDimensions(input_tex);if(gid.x>=extent.x||gid.y>=extent.y){return;}let p=vec2<i32>(gid.xy);textureStore(output_tex,p,vec4<f32>(reconstruct(p,extent),1.0));}
