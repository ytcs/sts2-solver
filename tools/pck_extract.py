#!/usr/bin/env python3
"""
Godot 4 PCK file extractor.
Extracts all files from a Godot 4 .pck file.
"""

import struct
import os
import sys
import hashlib

PCK_MAGIC = 0x43504447  # GDPC in little-endian

def read_pck(pck_path: str, out_dir: str, filter_fn=None):
    """Extract all (or filtered) files from a Godot 4 PCK."""
    with open(pck_path, "rb") as f:
        # Read header
        magic = struct.unpack("<I", f.read(4))[0]
        if magic != PCK_MAGIC:
            # PCK might be embedded at end of exe; for standalone .pck it starts at 0
            raise ValueError(f"Not a Godot PCK file (magic={magic:#x})")

        fmt_ver, major, minor, patch = struct.unpack("<4I", f.read(16))
        print(f"PCK format v{fmt_ver}, Godot {major}.{minor}.{patch}")
        
        # Reserved
        f.read(16 * 4)  # 16 uint32 reserved fields
        
        # File count
        file_count = struct.unpack("<I", f.read(4))[0]
        print(f"File count: {file_count}")
        
        # Read file index
        entries = []
        for _ in range(file_count):
            path_len = struct.unpack("<I", f.read(4))[0]
            path = f.read(path_len).decode("utf-8").rstrip("\x00")
            
            ofs, size = struct.unpack("<QQ", f.read(16))
            md5 = f.read(16)
            
            # Godot 4 has flags field
            if fmt_ver >= 2:
                flags = struct.unpack("<I", f.read(4))[0]
            else:
                flags = 0
            
            entries.append((path, ofs, size, md5, flags))
        
        # Extract files
        extracted = 0
        skipped = 0
        for path, ofs, size, md5, flags in entries:
            clean_path = path.lstrip("res://").lstrip("/")
            
            if filter_fn and not filter_fn(clean_path):
                skipped += 1
                continue
            
            out_path = os.path.join(out_dir, clean_path)
            os.makedirs(os.path.dirname(out_path), exist_ok=True)
            
            f.seek(ofs)
            data = f.read(size)
            
            with open(out_path, "wb") as out:
                out.write(data)
            
            extracted += 1
            if extracted <= 20 or extracted % 500 == 0:
                print(f"  [{extracted}] {clean_path} ({size} bytes)")
        
        print(f"\nExtracted: {extracted}, Skipped: {skipped}")
        return entries

def list_pck(pck_path: str):
    """List all files in a PCK without extracting."""
    with open(pck_path, "rb") as f:
        magic = struct.unpack("<I", f.read(4))[0]
        if magic != PCK_MAGIC:
            raise ValueError(f"Not a Godot PCK file (magic={magic:#x})")
        
        fmt_ver, major, minor, patch = struct.unpack("<4I", f.read(16))
        f.read(16 * 4)
        
        file_count = struct.unpack("<I", f.read(4))[0]
        print(f"PCK format v{fmt_ver}, Godot {major}.{minor}.{patch}, {file_count} files")
        
        paths = []
        for _ in range(file_count):
            path_len = struct.unpack("<I", f.read(4))[0]
            path = f.read(path_len).decode("utf-8").rstrip("\x00")
            ofs, size = struct.unpack("<QQ", f.read(16))
            md5 = f.read(16)
            if fmt_ver >= 2:
                flags = struct.unpack("<I", f.read(4))[0]
            else:
                flags = 0
            paths.append((path, size))
        
        return paths


if __name__ == "__main__":
    PCK = "/home/ytc/.local/share/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.pck"
    OUT = "/home/ytc/Projects/sts2-solver/data/pck_extracted"
    
    mode = sys.argv[1] if len(sys.argv) > 1 else "list"
    
    if mode == "list":
        paths = list_pck(PCK)
        print(f"\nAll {len(paths)} files:")
        for p, sz in sorted(paths):
            print(f"  {sz:>10} {p}")
    
    elif mode == "filter-list":
        # List only interesting content files (JSON, tres, etc.)
        paths = list_pck(PCK)
        interesting = [(p, sz) for p, sz in paths 
                       if any(p.endswith(ext) for ext in ['.json', '.tres', '.tscn', '.cfg', '.txt', '.csv'])]
        print(f"\nInteresting files ({len(interesting)}/{len(paths)}):")
        for p, sz in sorted(interesting):
            print(f"  {sz:>10} {p}")
    
    elif mode == "extract-data":
        # Extract only data files (json, cfg, etc.) - skip binaries/textures/audio
        SKIP_EXT = {'.png', '.jpg', '.webp', '.ogg', '.mp3', '.wav', '.ttf', '.otf', 
                    '.ctex', '.import', '.translation', '.atlastex', '.mesh',
                    '.scn', '.compiled'}
        SKIP_DIR = {'images', 'audio', 'sounds', 'music', 'fonts', 'shaders'}
        
        def want(path: str) -> bool:
            ext = os.path.splitext(path)[1].lower()
            if ext in SKIP_EXT:
                return False
            parts = path.split('/')
            if any(d in SKIP_DIR for d in parts):
                return False
            return True
        
        os.makedirs(OUT, exist_ok=True)
        read_pck(PCK, OUT, filter_fn=want)
    
    elif mode == "extract-all":
        os.makedirs(OUT, exist_ok=True)
        read_pck(PCK, OUT)
