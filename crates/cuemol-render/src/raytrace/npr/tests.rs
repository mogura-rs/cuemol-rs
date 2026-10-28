use super::*;
use params::{Kind, Layer};

#[test]
fn specs_are_atomic_and_preserve_layers_tone_and_ink() {
    for style in HATCH_STYLES {
        let look = Look::style(style).unwrap();
        let spec = look.spec();
        let mut restored = Look::default();
        restored.apply_spec(&spec, 7).unwrap();
        assert_eq!(restored.spec(), spec);
        let original = restored.clone();
        assert!(restored
            .apply_spec("layer:width=4\ntone:unknown=1", 7)
            .is_err());
        assert_eq!(original, restored);
        assert!(!look.layers.is_empty());
    }
    let mut look = Look::style("ink-cross").unwrap();
    look.apply_spec("layer:kind=dot,spacing=7,shape=4; tone:strength=0.6,curve=0.8; ink:base=albedo,inkcolor=#ff0000",7).unwrap();
    assert_eq!(look.layers.len(), 1);
    assert_eq!(look.layers[0].kind, Kind::Dot);
    assert_eq!(look.tone.strength, 0.6);
    assert_eq!(look.ink, [1., 0., 0.]);
    assert!(look.albedo_base);
}
#[test]
fn marks_are_deterministic_and_monotone_as_tone_darkens() {
    for kind in [Kind::Line, Kind::Dot, Kind::Stipple] {
        let layer = Layer {
            kind,
            jitter: 0.3,
            wobble: 0.7,
            wjitter: 0.2,
            slen: 17.,
            sgap: 3.,
            seed: 17,
            ..Layer::default()
        };
        let layer = NormalizedLayer::new(&layer, 2);
        for i in 0..80 {
            let x = i as f32 * 1.37 - 30.;
            let y = i as f32 * 0.83 - 17.;
            let mut previous = 0.;
            for j in 0..=20 {
                let tone = 1. - j as f32 / 20.;
                let c = layer.coverage(x, y, tone);
                assert!(c.is_finite() && (0. ..=1.).contains(&c));
                assert!(c + 1e-6 >= previous);
                assert_eq!(c, layer.coverage(x, y, tone));
                previous = c;
            }
        }
    }
}
#[test]
fn tone_shaping_and_ink_alpha_contracts_hold() {
    let p = HatchRenderParams {
        style: "ink-cross".into(),
        tone_spec: "tone:strength=0".into(),
        ..Default::default()
    };
    let h = ResolvedHatch::new(&p, 3).unwrap();
    assert_eq!(
        h.composite(12., 14., 0.2, [0.4; 3], [0.; 3], false),
        [1.; 3]
    );
    assert_eq!(h.composite(12., 14., 0.2, [0.4; 3], [0.; 3], true), [0.; 3]);
    let mut p = p;
    p.tone_spec = "ink:base=albedo; tone:strength=0".into();
    let h = ResolvedHatch::new(&p, 3).unwrap();
    assert_eq!(
        h.composite(12., 14., 0.1, [0.2, 0.4, 0.8], [0.; 3], false),
        [0.2, 0.4, 0.8]
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn procedural_marks_match_the_reference_cpp_kernel() {
    use std::{fs, path::PathBuf, process::Command};
    let root = std::env::var_os("UMBREON_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/umbreon-reference")
        });
    if !root.join("src/umbreon/npr/hatch_ink.hpp").is_file() {
        eprintln!("SKIP: set UMBREON_REF_DIR to the optional Umbreon reference checkout");
        return;
    }
    let work = std::env::temp_dir().join(format!("cuemol-hatch-oracle-{}", std::process::id()));
    fs::create_dir_all(&work).unwrap();
    let source = work.join("oracle.cpp");
    let executable = work.join("oracle");
    fs::write(
        &source,
        r#"
#include "npr/hatch_ink.hpp"
#include <iostream>
#include <iomanip>
int main(){
  std::cout<<std::setprecision(9);
  for(int kind=0;kind<3;++kind){
    umbreon::HatchLayer l;l.kind=static_cast<umbreon::LayerKind>(kind);
    l.angleDeg=17;l.spacingPx=13;l.subdiv=2;l.widthPx=2.3;l.dotScale=1.2;
    l.toneHi=0.97;l.toneLo=0.21;l.fadeInv=0;l.mark.seed=17;
    l.mark.jitter=0.3;l.mark.shapeExponent=3.7;l.mark.dotAspect=1.4;l.mark.dotAngleDeg=27;
    l.mark.wobbleAmpPx=0.7;l.mark.widthJitter=0.2;l.mark.strokeLenPx=17;l.mark.strokeGapPx=3;
    l.mark.angleJitterDeg=4;l.mark.strokeLenJitter=0.3;l.mark.toothAmp=0.1;
    auto r=umbreon::detail::hatchNormalizeLayer(l,2);
    for(int i=0;i<80;++i){float x=i*1.37f-30,y=i*0.83f-17;
      for(int j=0;j<=20;++j)std::cout<<umbreon::detail::hatchLayerInk(r,x,y,1-j/20.f)<<'\n';}
  }
}
"#,
    )
    .unwrap();
    let compiler = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    let output = Command::new(compiler)
        .args(["-std=c++17", "-O2", "-ffp-contract=off", "-I"])
        .arg(root.join("src/umbreon"))
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let values: Vec<f32> = text.lines().map(|s| s.parse().unwrap()).collect();
    let mut n = 0;
    for kind in [Kind::Line, Kind::Dot, Kind::Stipple] {
        let l = Layer {
            kind,
            angle: 17.,
            spacing: 13.,
            subdiv: 2,
            width: 2.3,
            dotscale: 1.2,
            tonehi: 0.97,
            tonelo: 0.21,
            fade: 0.,
            seed: 17,
            jitter: 0.3,
            shape: 3.7,
            aspect: 1.4,
            dotangle: 27.,
            wobble: 0.7,
            wjitter: 0.2,
            slen: 17.,
            sgap: 3.,
            anglejitter: 4.,
            lenjitter: 0.3,
            tooth: 0.1,
            ..Layer::default()
        };
        let r = NormalizedLayer::new(&l, 2);
        for i in 0..80 {
            for j in 0..=20 {
                let got = r.coverage(
                    i as f32 * 1.37 - 30.,
                    i as f32 * 0.83 - 17.,
                    1. - j as f32 / 20.,
                );
                assert!(
                    (got - values[n]).abs() < 5e-5,
                    "{kind:?} sample {i}/{j}: Rust {got}, C++ {}",
                    values[n]
                );
                n += 1;
            }
        }
    }
    assert_eq!(n, values.len());
    fs::remove_dir_all(work).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn all_style_composites_match_the_reference_renderer() {
    use std::{fs, path::PathBuf, process::Command};
    let root = std::env::var_os("UMBREON_REF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/umbreon-reference")
        });
    let reference = root.join("src/umbreon");
    if !reference.join("npr/hatch_shade.cpp").is_file() {
        eprintln!("SKIP: set UMBREON_REF_DIR to the optional Umbreon reference checkout");
        return;
    }
    let work = std::env::temp_dir().join(format!("cuemol-hatch-composite-{}", std::process::id()));
    fs::create_dir_all(work.join("tbb")).unwrap();
    fs::write(work.join("tbb/blocked_range.h"),"#pragma once\nnamespace tbb {template<class T> class blocked_range {T a,b;public:blocked_range(T x,T y):a(x),b(y){} T begin()const{return a;} T end()const{return b;}};}\n").unwrap();
    fs::write(work.join("tbb/parallel_for.h"),"#pragma once\nnamespace tbb {template<class R,class F> void parallel_for(R r,F f){f(r);}}\n").unwrap();
    let source = work.join("oracle.cpp");
    let executable = work.join("oracle");
    fs::write(&source,r#"
#include "npr/hatch_shade.hpp"
#include <iostream>
#include <iomanip>
#include <cmath>
namespace umbreon {float srgbEncodeF(float v){v=std::max(0.f,std::min(1.f,v));return v<=0.0031308f?12.92f*v:1.055f*std::pow(v,1.f/2.4f)-0.055f;}}
int main(int argc,char**argv){
  umbreon::HatchOptions opt;opt.enable=true;
  if(argc!=2||!umbreon::applyHatchStyle(opt,argv[1]))return 1;
  const int w=48,h=32;float rgba[w*h*4],tone[w*h],mask[w*h],albedo[w*h*3];
  for(int y=0;y<h;++y)for(int x=0;x<w;++x){int p=y*w+x;tone[p]=0.02f+1.2f*x/(w-1);mask[p]=1.f;
    float c[3]={0.1f+0.6f*y/(h-1),0.4f,0.8f};
    for(int k=0;k<3;++k){albedo[p*3+k]=c[k];rgba[p*4+k]=opt.base==umbreon::HatchBase::Albedo?c[k]:opt.paperColor[k];}rgba[p*4+3]=1.f;}
  umbreon::applyHatch(w,h,rgba,tone,mask,albedo,opt);
  std::cout<<std::setprecision(9);for(float v:rgba)std::cout<<v<<'\n';
}
"#).unwrap();
    let compiler = std::env::var("CXX").unwrap_or_else(|_| "c++".into());
    let output = Command::new(compiler)
        .args(["-std=c++17", "-O2", "-ffp-contract=off", "-I"])
        .arg(&reference)
        .arg("-I")
        .arg(&work)
        .arg(&source)
        .arg(reference.join("npr/hatch_shade.cpp"))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for style in HATCH_STYLES {
        let output = Command::new(&executable).arg(style).output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let values: Vec<f32> = text.lines().map(|s| s.parse().unwrap()).collect();
        assert_eq!(values.len(), 48 * 32 * 4);
        let hatch = ResolvedHatch::new(
            &HatchRenderParams {
                style: style.into(),
                ..Default::default()
            },
            1,
        )
        .unwrap();
        for y in 0..32 {
            for x in 0..48 {
                let tone = 0.02 + 1.2 * x as f32 / 47.;
                let albedo = [0.1 + 0.6 * y as f32 / 31., 0.4, 0.8];
                let got =
                    hatch.composite(x as f32 + 0.5, y as f32 + 0.5, tone, albedo, [0.; 3], false);
                for k in 0..3 {
                    let want = values[(y * 48 + x) * 4 + k];
                    assert!(
                        (got[k] - want).abs() < 7e-5,
                        "{style} ({x},{y}) channel {k}: Rust {}, C++ {want}",
                        got[k]
                    );
                }
            }
        }
    }
    fs::remove_dir_all(work).unwrap();
}
