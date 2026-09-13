#!/usr/bin/env python3
"""Regenerate crates/brokey-core/tests/vercmp.rs from pacman's own `vercmp`.

Run on an Arch machine. The table is the truth; the Rust is what has to agree.
"""
import itertools, pathlib, random, subprocess, sys

VERSIONS = ["1.0","1.0a","1.0.1","1:1.0","2.0","1.0-1","1.0-2","1.0~rc1","1.0rc1","1.0.0","1","0.9","1.0beta","1.0b","r16.961702d-1","r17.0-1","20260821-1","2.6.1.r273.gd3ab993-1","2.6.1-1","1.0.0.87-3","1.0.0.87-4","3.2.4-1","3.2.4-1.1","1.98.1-1.1","1:1.98.1-1","0.28.1-1","0.28.1.r0.g12ab-1","5.05-6.1","5.05-6","2.1.0-2","2.1.0+git-1","1.18.2-1","1:1.18.2-1.1","3:26.2.2-2","610.57.04-1","610.57.4-1","1.0.0","1.0.0alpha","1.0.0_beta","1.0.0.beta","1.0.0-beta","1.0~","1.0~~","1.0~a","1.0.a","1.0.0.0","001.002","1.2","1.02","1.10","1.9","1.0+","1.0+1","1.0-0.1","1.0-1.a","2020.01.01","2020.1.1"]

def main():
    random.seed(4)
    pairs = list(itertools.combinations(VERSIONS, 2))
    random.shuffle(pairs)
    rows = []
    for a, b in pairs[:400]:
        r = subprocess.run(["vercmp", a, b], capture_output=True, text=True, check=True).stdout.strip()
        rows.append(f'    ("{a}", "{b}", {r}),')
    out = pathlib.Path(__file__).resolve().parent.parent / "crates/brokey-core/tests/vercmp.rs"
    text = out.read_text()
    start = text.index("const TABLE")
    end = text.index("];", start) + 2
    text = text[:start] + "const TABLE: &[(&str, &str, i8)] = &[\n" + "\n".join(rows) + "\n];" + text[end:]
    out.write_text(text)
    print(f"wrote {len(rows)} rows to {out}")

if __name__ == "__main__":
    sys.exit(main())
