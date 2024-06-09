#!/usr/bin/env python3
#
# This script should be used to reroot unrooted trees
#

from pathlib import Path

from ete3 import Tree


def __main(
    input_tree_file: Path,
    output_tree_file: Path,
) -> None:

    tree = Tree(str(input_tree_file), format=1)
    tree.set_outgroup(tree.get_midpoint_outgroup())
    tree.write(outfile=str(output_tree_file), format=1)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Reroot unrooted trees")
    parser.add_argument("input_tree_file", type=Path, help="Input tree file")
    parser.add_argument("output_tree_file", type=Path, help="Output tree file")

    args = parser.parse_args()
    __main(args.input_tree_file, args.output_tree_file)
