# tendon-api

Simple datastructures describing a rust program's interface: types, function signatures, consts, etc.

Produced and consumed by other `tendon` crates.

# Design 1

tree of structures

# Design 2

modules:
map from path to module

macros:
map from path to macro

types:
map from path to type

traits:
map from path to trait

symbols:
map from path to symbol

track changes?

# Design 3

everything gets an int ID
on that ID we hang:
full path
unresolved dependencies