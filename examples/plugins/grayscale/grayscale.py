import sys

from PIL import Image

src, dst = sys.argv[1], sys.argv[2]
img = Image.open(src)
img.convert("LA" if "A" in img.getbands() else "L").save(dst, format="PNG")
