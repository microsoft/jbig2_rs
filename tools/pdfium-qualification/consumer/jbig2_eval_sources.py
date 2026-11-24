from pathlib import Path
import sys

for path in sorted(Path(sys.argv[1]).rglob("*.rs")):
    print(str(path.resolve()))
