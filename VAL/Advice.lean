/-!
# Advice propositions and the φ/ψ mapping

Definitions 6.1 and 6.2 of Howey, Long, and Fox, *VAL: Automatic Plan
Validation, Continuous Effects and Mixed Initiative Planning using PDDL*.

Unlike `VAL.Execution`, this module's central content -- the mapping `φ`/`ψ`
from a PDDL precondition and a state to an advice proposition -- is fully
concrete and structurally recursive, needing no abstract semantics interface
beyond "what makes a literal or comparison true in a state", which is supplied
directly rather than bundled into a carrier structure. The transcribed
equations (re-verified against the source PDF before encoding, not guessed):

```
φ(P,S) := if S ⊨ P then no advice else set P to true            [literal]
φ(P,S) := if P is an unsatisfied comparison then satisfy P       [comparison]
φ(∧ᵢXᵢ,S) := ∧ᵢ φ(Xᵢ,S), for each unsatisfied Xᵢ in S
φ(∨ᵢXᵢ,S) := ∨ᵢ φ(Xᵢ,S), for each unsatisfied Xᵢ in S
φ(X→Y,S) := φ(¬X∨Y,S)
φ(¬Q,S) := ψ(Q,S)

ψ(Q,S) := if Q ⊭ S then no advice else set Q to false            [literal]
ψ(Q,S) := if Q is a satisfied comparison then do not satisfy Q   [comparison]
ψ(∧ᵢXᵢ,S) := ∨ᵢ φ(¬Xᵢ,S), for each satisfied Xᵢ in S
ψ(∨ᵢXᵢ,S) := ∧ᵢ φ(¬Xᵢ,S), for each satisfied Xᵢ in S
ψ(X→Y,S) := φ(X∧¬Y,S)
ψ(¬Q′,S) := φ(Q′,S)
```

## A termination note worth being explicit about

`φ(X→Y,S) := φ(¬X∨Y,S)` and `ψ(X→Y,S) := φ(X∧¬Y,S)` are, read literally,
recursive calls on a *larger* term (`¬X∨Y` has strictly more nodes than
`X→Y`) -- the paper's own remark that "the map φ is well defined since PDDL
preconditions and states are finite" glosses over this. It is well founded,
just not by naive structural recursion on the rewritten term: unfolding one
more step shows every call these two equations actually make lands on `X`,
`Y`, or `¬X`/`¬Y` directly -- genuine subterms of `X→Y` -- because whichever
branch of the constructed disjunction/conjunction is "unsatisfied" is
determined entirely by `X`/`Y`'s own truth values at the point these equations
fire. The two `implies` cases below are written unfolded one step, calling
`φ`/`ψ` only on `x`, `y` themselves, which is what makes them accepted by
Lean's termination checker without weakening what they compute.

The `and`/`or` cases need a second termination accommodation: Lean's automatic
structural-recursion checker cannot see through a higher-order combinator like
`List.all`/`List.filter` applied to a partially-applied recursive call, so the
"map/filter over the child list" operations are written as their own
explicitly recursive list-processing functions (`SatisfiesAll`, `SatisfiesAny`,
`phiUnsatisfied`, `psiSatisfied`) rather than via those combinators -- the
standard idiom for recursion into a nested inductive's `List`-typed field.
-/

namespace VAL
namespace Advice

/-- A PDDL precondition, generic over an atom type (what a literal names) and
a comparison type (a numeric comparison over PNEs). Explicit recursive syntax,
matching the six forms Definition 6.2 dispatches φ/ψ on: literal, comparison,
conjunction, disjunction, implication, negation. -/
inductive PddlCondition (Atom Cmp : Type u)
  | literal (atom : Atom)
  | comparison (cmp : Cmp)
  | and (xs : List (PddlCondition Atom Cmp))
  | or (xs : List (PddlCondition Atom Cmp))
  | implies (x y : PddlCondition Atom Cmp)
  | not (x : PddlCondition Atom Cmp)

/-- Definition 6.1 (Advice Proposition). Boolean set/unset for a literal,
satisfy a comparison, conjunction, disjunction, or no advice (the empty case).

One constructor beyond the paper's six-item prose list: `doNotSatisfy`.
Definition 6.2's ψ names "do not satisfy `Q`" as a distinct comparison-case
output, but Definition 6.1's own enumeration does not separately list it --
presumably folded informally into "satisfy a comparison" under an assumed
comparison-negation the paper never supplies generically. Modeled as its own
constructor rather than silently assuming `Cmp` is closed under negation; a
documented reading, not a smoothing-over. -/
inductive AdviceProposition (Atom Cmp : Type u)
  | setTrue (atom : Atom)
  | setFalse (atom : Atom)
  | satisfy (cmp : Cmp)
  | doNotSatisfy (cmp : Cmp)
  | conj (aps : List (AdviceProposition Atom Cmp))
  | disj (aps : List (AdviceProposition Atom Cmp))
  | none

variable {Atom Cmp State : Type u}

mutual

/-- Whether a literal/comparison holds in a state. Supplied directly (as two
plain functions) rather than bundled into a carrier structure, unlike
`VAL.Execution.ActSemantics`: nothing about `φ`/`ψ`'s own recursive structure
depends on what these functions compute, only that they exist and return
`Bool`. -/
def Satisfies (atomHolds : Atom → State → Bool) (cmpHolds : Cmp → State → Bool)
    (S : State) : PddlCondition Atom Cmp → Bool
  | .literal a => atomHolds a S
  | .comparison c => cmpHolds c S
  | .and xs => SatisfiesAll atomHolds cmpHolds S xs
  | .or xs => SatisfiesAny atomHolds cmpHolds S xs
  | .implies x y =>
      !(Satisfies atomHolds cmpHolds S x) || Satisfies atomHolds cmpHolds S y
  | .not x => !(Satisfies atomHolds cmpHolds S x)

/-- Whether every condition in a list is satisfied. -/
def SatisfiesAll (atomHolds : Atom → State → Bool) (cmpHolds : Cmp → State → Bool)
    (S : State) : List (PddlCondition Atom Cmp) → Bool
  | [] => true
  | x :: rest =>
      Satisfies atomHolds cmpHolds S x && SatisfiesAll atomHolds cmpHolds S rest

/-- Whether some condition in a list is satisfied. -/
def SatisfiesAny (atomHolds : Atom → State → Bool) (cmpHolds : Cmp → State → Bool)
    (S : State) : List (PddlCondition Atom Cmp) → Bool
  | [] => false
  | x :: rest =>
      Satisfies atomHolds cmpHolds S x || SatisfiesAny atomHolds cmpHolds S rest

end

mutual

/-- Definition 6.2's `φ`. -/
def phi (atomHolds : Atom → State → Bool) (cmpHolds : Cmp → State → Bool)
    (S : State) : PddlCondition Atom Cmp → AdviceProposition Atom Cmp
  | .literal a => if atomHolds a S then .none else .setTrue a
  | .comparison c => if cmpHolds c S then .none else .satisfy c
  | .and xs =>
      if SatisfiesAll atomHolds cmpHolds S xs then .none
      else .conj (phiUnsatisfied atomHolds cmpHolds S xs)
  | .or xs =>
      if SatisfiesAny atomHolds cmpHolds S xs then .none
      else .disj (phiUnsatisfied atomHolds cmpHolds S xs)
  | .implies x y =>
      -- φ(X→Y,S) := φ(¬X∨Y,S), unfolded one step (see the module docstring):
      -- reached only when X→Y is unsatisfied, i.e. X holds and Y does not, so
      -- both ¬X and Y are themselves unsatisfied and both contribute.
      if !(Satisfies atomHolds cmpHolds S x) || Satisfies atomHolds cmpHolds S y
      then .none
      else .disj [psi atomHolds cmpHolds S x, phi atomHolds cmpHolds S y]
  | .not q => psi atomHolds cmpHolds S q

/-- Definition 6.2's `ψ`, defined only for `Q` such that `¬Q` is being fixed by
`φ`'s `not` case above -- i.e. exactly when `Q` itself is satisfied, which
each equation checks for itself. -/
def psi (atomHolds : Atom → State → Bool) (cmpHolds : Cmp → State → Bool)
    (S : State) : PddlCondition Atom Cmp → AdviceProposition Atom Cmp
  | .literal a => if !(atomHolds a S) then .none else .setFalse a
  | .comparison c => if !(cmpHolds c S) then .none else .doNotSatisfy c
  | .and xs =>
      if !(SatisfiesAll atomHolds cmpHolds S xs) then .none
      else .disj (psiSatisfied atomHolds cmpHolds S xs)
  | .or xs =>
      if !(SatisfiesAny atomHolds cmpHolds S xs) then .none
      else .conj (psiSatisfied atomHolds cmpHolds S xs)
  | .implies x y =>
      -- ψ(X→Y,S) := φ(X∧¬Y,S), unfolded one step: reached only when X→Y is
      -- satisfied; the target X∧¬Y's guard and construction are inlined over
      -- x and ¬y directly rather than built as a fresh, larger term.
      if Satisfies atomHolds cmpHolds S x && !(Satisfies atomHolds cmpHolds S y)
      then .none
      else
        .conj
          ((if !(Satisfies atomHolds cmpHolds S x)
              then [phi atomHolds cmpHolds S x] else []) ++
           (if Satisfies atomHolds cmpHolds S y
              then [psi atomHolds cmpHolds S y] else []))
  | .not q => phi atomHolds cmpHolds S q

/-- `φ` mapped over exactly the unsatisfied conditions of a list, used by both
of `φ`'s `and`/`or` cases -- they build the same underlying list, differing
only in the outer `conj`/`disj` wrapper. -/
def phiUnsatisfied (atomHolds : Atom → State → Bool)
    (cmpHolds : Cmp → State → Bool) (S : State) :
    List (PddlCondition Atom Cmp) → List (AdviceProposition Atom Cmp)
  | [] => []
  | x :: rest =>
      if Satisfies atomHolds cmpHolds S x then
        phiUnsatisfied atomHolds cmpHolds S rest
      else
        phi atomHolds cmpHolds S x :: phiUnsatisfied atomHolds cmpHolds S rest

/-- `ψ` mapped over exactly the satisfied conditions of a list, used by both
of `ψ`'s `and`/`or` cases. -/
def psiSatisfied (atomHolds : Atom → State → Bool)
    (cmpHolds : Cmp → State → Bool) (S : State) :
    List (PddlCondition Atom Cmp) → List (AdviceProposition Atom Cmp)
  | [] => []
  | x :: rest =>
      if Satisfies atomHolds cmpHolds S x then
        psi atomHolds cmpHolds S x :: psiSatisfied atomHolds cmpHolds S rest
      else
        psiSatisfied atomHolds cmpHolds S rest

end

end Advice
end VAL
