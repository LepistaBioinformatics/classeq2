#!/usr/bin/env python3
#
# This script should be used to reroot unrooted trees
#

from pathlib import Path

from ete3 import Tree


def __main(
    input_tree_file: Path,
    output_tree_file: Path,
    format: int = 0,
) -> None:

    tree = Tree(str(input_tree_file), format=format)
    tree.set_outgroup(tree.get_midpoint_outgroup())
    tree.write(outfile=str(output_tree_file), format=1)


if __name__ == "__main__":
    from argparse import ArgumentParser, RawTextHelpFormatter

    parser = ArgumentParser(
        description="Reroot unrooted trees",
        formatter_class=RawTextHelpFormatter,
    )

    parser.add_argument("input_tree_file", type=Path, help="Input tree file")
    parser.add_argument("output_tree_file", type=Path, help="Output tree file")

    parser.add_argument(
        "-f",
        "--format",
        type=int,
        default=0,
        help="""
Format of the input tree file (default: 0)\n

======  ==============================================
FORMAT  DESCRIPTION
======  ==============================================
0        flexible with support values
1        flexible with internal node names
2        all branches + leaf names + internal supports
3        all branches + all names
4        leaf branches + leaf names
5        internal and leaf branches + leaf names
6        internal branches + leaf names
7        leaf branches + all names
8        all names
9        leaf names
100      topology only
======  ==============================================
""",
    )

    args = parser.parse_args()
    __main(args.input_tree_file, args.output_tree_file, args.format)
