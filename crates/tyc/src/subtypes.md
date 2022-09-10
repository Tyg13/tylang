# Algebraic subtyping a la Stephen Dolan

The idea is that the traditional formulation of types is roughly as follows

Define

```
types = boolean type + function types + record types
```
(where `+` is used to denote disjoint union)

We seek to define an algebra that allows for subtyping on `types`.
A _subtype relation_ is defined as a _partial order_ where

a <: b denotes that a can be used in the context of b

Given this structure, we can see `types` is a disjoint union of posets.
It would be useful if we could make it a _lattice_, which just means
that, for any two elements we have a unique infimum (meet, `^`) and supremum
(join, `v`).

Traditionally, this is done by affixing two types: top and bottom,

```
types = boolean type + function types + record types + top + bottom
```

Where `top` denotes the supertype of all types (`a <: top forall a in types`)
and `bottom` denotes the subtype of all types (`bottom <: a forall a in
types`)

But this leads to issues with extensibility (adding new types should not
break type rules) This is because we're imposing structure on the disjoint
union to make it a lattice, rather than treating the constituent posets as
lattices.

Additionally, the traditional treatment of type variables as quantifiers over
the set of types leads to issues with extensibility.

A better formulation, more algebraicly motivated is

```
types = boolean type + function types + record types + type variables
```

We require each of the constituent sets to be _distributive lattices_, that
is their meet and join operations satisfy

```
a ^ (b v c) = (a v b) ^ (a v c)
a v (b ^ c) = (a ^ b) v (a ^ c)
```

The set `T_s` of _simple types_ can simply be defined as

```
t in T_s ::= bool | t_1 -> t_2 | {l_1: t_1, ... , l_n: t_n}
```

Where `l_n in L`, the set of record labels. Record types are defined as a
partial function from record labels to values, i.e. for some `RecordT`

```
domain of RecordT = {l_1, l_2, ..., l_n}
RecordT(l_n) = t_n
```

`T_s` can be seen as the initial algebra of a functor `F_s`

We define it as the disjoint union of its constituent functors

```
Bool_s(A) = 1
Func_s(A) = A x A
Rec_s(A) = (A + 1)^L
```

since there is only one boolean type, functions are given by
their domain and range, and record types (which are partial
functions from `L` to `A`), can be seen as total functions
from `L` to `A + 1` by free construction

Note that `(A)^L` denotes iterated product i.e. `A x A x ... A`

Hence the functor `F_s` of which `T_s` is the initial algebra is

```
F_s(A) = Bool_s(A) + Func_s(A) + Rec_s(A)
```
