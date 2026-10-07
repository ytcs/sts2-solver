"""Read-only Godot 4 PCK reader (pack format 2 and 3) and CompressedTexture2D (`.ctex`) decoder.

The pack is untrusted data: this module parses its directory and fixed binary headers and copies bytes out; nothing in it is executed.
Images come out as the embedded WebP / PNG blob (no re-encode), or through Pillow for raw and VRAM-compressed (BC1/BC3/BC7) textures.
Used once by `tools/dashboard/extract_assets.py`, never by the dashboard's polling loop.
"""
import re
import struct

MAGIC = b"GDPC"
PACK_DIR_ENCRYPTED = 1
PACK_REL_FILEBASE = 2
FILE_ENCRYPTED = 1
FILE_REMOVAL = 2


class Pack:
    def __init__(self, path):
        self.path = path
        self.f = open(path, "rb")
        self.files = {}  # res path (without res://) -> (absolute offset, size)
        self._read_dir()

    def close(self):
        self.f.close()

    def _read_dir(self):
        f = self.f
        head = f.read(4 + 4 * 4)
        if head[:4] != MAGIC:
            raise ValueError("not a Godot PCK (no GDPC magic at offset 0)")
        fmt, major, minor, patch = struct.unpack_from("<4I", head, 4)
        if fmt not in (2, 3):
            raise ValueError(f"unsupported pack format {fmt} (Godot {major}.{minor}.{patch})")
        flags, file_base = struct.unpack("<IQ", f.read(12))
        if flags & PACK_DIR_ENCRYPTED:
            raise ValueError("the pack directory is encrypted")
        if fmt == 3:
            (dir_offset,) = struct.unpack("<Q", f.read(8))
            f.seek(dir_offset)
        else:
            f.read(16 * 4)  # reserved
        base = file_base if flags & PACK_REL_FILEBASE else 0
        (count,) = struct.unpack("<I", f.read(4))
        if count > 2_000_000:
            raise ValueError(f"implausible file count {count}")
        for _ in range(count):
            (plen,) = struct.unpack("<I", f.read(4))
            if plen > 4096:
                raise ValueError(f"implausible path length {plen}")
            name = f.read(plen).rstrip(b"\0").decode("utf-8", "replace")
            off, size = struct.unpack("<QQ", f.read(16))
            f.read(16)  # md5
            (fflags,) = struct.unpack("<I", f.read(4))
            if fflags & (FILE_ENCRYPTED | FILE_REMOVAL):
                continue
            self.files[name[6:] if name.startswith("res://") else name] = (base + off, size)

    def read(self, name, limit=64 << 20):
        off, size = self.files[name]
        if size > limit:
            raise ValueError(f"{name}: {size} bytes is over the limit")
        self.f.seek(off)
        return self.f.read(size)

    def text(self, name):
        return self.read(name, 8 << 20).decode("utf-8", "replace")

    def imported(self, src):
        """The imported file (`.godot/imported/...`) of a source asset, from its `.import` remap (`path=`, `path.s3tc=`, `path.bptc=` ...), or None."""
        imp = src + ".import"
        if imp not in self.files:
            return None
        paths = re.findall(r'^path(?:\.(\w+))?="res://([^"]+)"', self.text(imp), re.M)
        order = {"": 0, "bptc": 1, "s3tc": 2}  # desktop formats first; never etc2/astc
        paths = sorted((p for p in paths if p[0] in order), key=lambda p: order[p[0]])
        return paths[0][1] if paths and paths[0][1] in self.files else None


# ---------------------------------------------------------------- .ctex (CompressedTexture2D)

DATA_FORMAT_IMAGE, DATA_FORMAT_PNG, DATA_FORMAT_WEBP = 0, 1, 2
# Godot Image::Format -> (Pillow raw mode or bcn index, bytes per pixel or None for block formats)
RAW = {0: ("L", 1), 1: ("LA", 2), 4: ("RGB", 3), 5: ("RGBA", 4)}
BCN = {17: 1, 18: 2, 19: 3, 22: 7}  # DXT1, DXT3, DXT5, BPTC_RGBA


def ctex(data):
    """The first mip of a .ctex: ('webp' | 'png', encoded bytes) when the texture embeds a file, else ('image', a Pillow RGBA image).
    Raises ValueError for formats this reader does not decode."""
    if data[:4] != b"GST2":
        raise ValueError("not a GST2 texture")
    p = 36  # magic, version, width, height, flags, mipmap limit, 3 reserved
    dfmt, w, h, _mips, ifmt = struct.unpack_from("<IHHII", data, p)
    p += 16
    if dfmt in (DATA_FORMAT_PNG, DATA_FORMAT_WEBP):
        (size,) = struct.unpack_from("<I", data, p)
        blob = data[p + 4:p + 4 + size]
        if blob[:4] == b"RIFF" and blob[8:12] == b"WEBP":
            return "webp", blob
        if blob[:8] == b"\x89PNG\r\n\x1a\n":
            return "png", blob
        raise ValueError("embedded image is neither PNG nor WebP")
    if dfmt != DATA_FORMAT_IMAGE:
        raise ValueError(f"data format {dfmt} not supported")
    from PIL import Image
    raw = data[p:]
    if ifmt in RAW:
        mode, bpp = RAW[ifmt]
        return "image", Image.frombytes(mode, (w, h), raw[:w * h * bpp]).convert("RGBA")
    if ifmt in BCN:
        n = BCN[ifmt]
        size = ((w + 3) // 4) * ((h + 3) // 4) * (8 if n == 1 else 16)
        return "image", Image.frombytes("RGBA", (w, h), raw[:size], "bcn", n)
    raise ValueError(f"image format {ifmt} not supported")


def to_image(kind, payload):
    """A Pillow image from `ctex()`'s result."""
    if kind == "image":
        return payload
    import io
    from PIL import Image
    return Image.open(io.BytesIO(payload)).convert("RGBA")
