"""C3 of docs/decisions/2026-10-07-floor-audit.md: pull every receipt blob out
of every ref without merging or checking out anything.

`git ls-tree -r <ref>` over all 110 local heads and all 30 remote heads, kept
to `docs/research/runs/`, `docs/decisions/` and any `receipts/` directory, then
de-duplicated by blob SHA so a receipt copied onto 110 branches is read once.
Writes `<out>/inventory.tsv` (ref, sha, path) and `<out>/blobs/<sha>.txt`.
"""
import os
import subprocess
import sys

REPO = sys.argv[1] if len(sys.argv) > 1 else "E:/rust/fd-floor-audit"
OUT = sys.argv[2]
KEEP = (".txt", ".md", ".log")


def git(*a):
    return subprocess.run(["git", *a], cwd=REPO, capture_output=True,
                          text=True, errors="replace").stdout


def wanted(p):
    return (p.startswith("docs/research/runs/") or p.startswith("docs/decisions/")
            or "receipts/" in p)


def main():
    os.makedirs(os.path.join(OUT, "blobs"), exist_ok=True)
    refs = [r for r in git("for-each-ref", "--format=%(refname:short)",
                           "refs/heads", "refs/remotes").split()
            if not r.endswith("HEAD")]
    rows, shas = [], {}
    for ref in sorted(set(refs)):
        for line in git("ls-tree", "-r", ref).splitlines():
            meta, _, path = line.partition("\t")
            parts = meta.split()
            if len(parts) < 3 or parts[1] != "blob":
                continue
            if not wanted(path):
                continue
            rows.append((ref, parts[2], path))
            if path.endswith(KEEP):
                shas.setdefault(parts[2], path)
    with open(os.path.join(OUT, "inventory.tsv"), "w", encoding="utf-8") as fh:
        for r in rows:
            fh.write("\t".join(r) + "\n")

    p = subprocess.Popen(["git", "cat-file", "--batch"], cwd=REPO,
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    for sha in shas:
        p.stdin.write((sha + "\n").encode())
        p.stdin.flush()
        size = int(p.stdout.readline().decode().split()[2])
        data = p.stdout.read(size)
        p.stdout.read(1)
        with open(os.path.join(OUT, "blobs", sha + ".txt"), "wb") as fh:
            fh.write(data)
    p.stdin.close()
    print(f"refs scanned: {len(set(refs))}")
    print(f"(ref, blob, path) tuples kept: {len(rows)}")
    print(f"distinct text blobs extracted: {len(shas)}")


if __name__ == "__main__":
    main()
