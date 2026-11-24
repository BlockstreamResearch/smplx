#!/usr/bin/env python3
"""Print real pinned compiler diagnostics without requiring generated bindings."""
import argparse
import os
from pathlib import Path
import subprocess

RUST = r'''
use simplex::simplicityhl::{TemplateAst, UnstableFeatures};
use simplex::simplicityhl::ast::ElementsJetHinter;
fn main() {
    let path = std::env::args().nth(1).expect("source file argument");
    let source = std::fs::read_to_string(&path).expect("read source file");
    match TemplateAst::new_with_unstable(
        source.as_str(), &UnstableFeatures::all(), Box::new(ElementsJetHinter)
    ) {
        Ok(_) => println!("Parsed and typechecked {path}; now rebuild with Simplex and run execution tests."),
        Err(error) => { eprintln!("{path}: {error}"); std::process::exit(1); }
    }
}
'''
MANIFEST = '''[package]
name = "simplex-source-check"
version = "0.1.0"
edition = "2024"
[dependencies]
smplx-std = "=0.2.0"
[workspace]
'''

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('source',type=Path)
    p.add_argument('--work-dir',type=Path,default=Path('.simplex-diagnostics'))
    a=p.parse_args()
    source=a.source.resolve(strict=True)
    work=a.work_dir.resolve()
    marker=work/'.checker-owned'
    if work.exists() and not marker.exists():
        p.error('Work directory already exists and was not created by this checker; choose another --work-dir.')
    (work/'src').mkdir(parents=True,exist_ok=True)
    marker.touch()
    (work/'Cargo.toml').write_text(MANIFEST)
    (work/'src/main.rs').write_text(RUST)
    env=dict(os.environ,CARGO_NET_OFFLINE='true')
    env.setdefault('CARGO_TARGET_DIR',str(Path.cwd()/'target'))
    result=subprocess.run(['cargo','run','--offline','--quiet','--manifest-path',str(work/'Cargo.toml'),'--',str(source)],env=env)
    raise SystemExit(result.returncode)

if __name__=='__main__':main()
