// Drop build artefacts old enough that nothing can still be asking for them.
//
// `emptyOutDir` is off on purpose: a tab opened before a rebuild still resolves
// the hashed files its own index.html names, and wiping the directory on every
// build turns that tab into a blank page. The cost is that nothing ever removed
// the old ones - a day of iterating left 104 bundles and 87 MB in `dist`, which
// is then 87 MB shipped to a server to serve 3 MB of them.
//
// The rule is a COUNT and not an age. An age looks more principled and bounds
// nothing: a day of heavy iteration produces thirty builds inside any window
// you would care to pick, and the directory grows anyway. Keeping the newest
// KEEP generations caps the directory no matter how fast anyone builds, and
// twelve of them is far more protection than an open tab has ever needed -
// fd-api serves index.html with no-cache, so any reload is already current and
// the only thing at risk is a lazy chunk fetched by a tab nobody has touched
// across twelve rebuilds.
//
// Whatever the current index.html names is kept regardless of its place in
// that order, because it is what a fresh visitor is about to ask for.
import { readdir, readFile, stat, unlink } from 'node:fs/promises'
import path from 'node:path'

const DIST = path.resolve(import.meta.dirname, '..', 'dist')
const ASSETS = path.join(DIST, 'assets')
const KEEP = 12

const html = await readFile(path.join(DIST, 'index.html'), 'utf8')
const referenced = new Set([...html.matchAll(/assets\/([A-Za-z0-9_.-]+)/g)].map((m) => m[1]))

// Grouped by the part before the content hash, so "the newest twelve" means
// twelve generations of each bundle rather than twelve files total - which
// would otherwise keep twelve copies of the main chunk and none of the fonts.
const byFamily = new Map()
for (const name of await readdir(ASSETS)) {
  const family = name.replace(/-[A-Za-z0-9_-]{6,}(\.\w+)$/, '$1')
  const info = await stat(path.join(ASSETS, name))
  if (!byFamily.has(family)) byFamily.set(family, [])
  byFamily.get(family).push({ name, mtime: info.mtimeMs, size: info.size })
}

let removed = 0
let freed = 0
let kept = 0
for (const files of byFamily.values()) {
  files.sort((a, b) => b.mtime - a.mtime)
  for (const [i, f] of files.entries()) {
    if (i < KEEP || referenced.has(f.name)) {
      kept++
      continue
    }
    await unlink(path.join(ASSETS, f.name))
    removed++
    freed += f.size
  }
}

console.log(
  removed > 0
    ? `prune-dist: removed ${removed} stale asset(s), ${(freed / 1048576).toFixed(1)} MB; kept ${kept}`
    : `prune-dist: nothing to remove (${kept} assets)`,
)
