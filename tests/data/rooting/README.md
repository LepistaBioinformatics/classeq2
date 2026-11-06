# Rooting fixtures

Inputs and reference outputs for `core/tests/rooting_ete3.rs`. That test
compares `cls root-tree` (midpoint) with the former `scripts/root_tree.py`,
which was removed when the native command shipped (feature 004-tree-rooting,
decision D4).

| File | Content |
|---|---|
| `*.input.nwk` | Unrooted trees: three random synthetic trees (8, 20 and 50 leaves, seeds 1 to 3), the PhyML GAPDH tree from `core/src/tests/data/colletotrichum-acutatom-complex/`, and the F-2 example (midpoint exactly on a node). |
| `*.ete3.nwk` | The same trees rooted by `root_tree.py`. |

The `*.ete3.nwk` files were generated on 2026-11-06 by running the script
unchanged, with ete3 3.1.3 on Python 3.14.4. Running ete3 3.1.3 on Python 3.13
or newer requires the `legacy-cgi` and `numpy` packages.

```bash
for f in *.input.nwk; do
    python scripts/root_tree.py "$f" "${f%.input.nwk}.ete3.nwk"
done
```

## Expected differences

- **Supports.** The script writes with ete3 `format=1`, which drops every
  support value (finding F-1). The native command keeps them.
- **Root branch lengths.** ete3 splits the chosen root edge 50/50. The native
  command places the root at the length midpoint (finding F-2). When the
  midpoint falls exactly on a node, ete3 uses the next edge, while the native
  command roots there with a zero-length edge.

On every fixture the rooted topology (the set of clades) is identical.
