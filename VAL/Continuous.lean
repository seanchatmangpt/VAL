import Mathlib.Analysis.Calculus.Deriv.Add
import Mathlib.Analysis.Calculus.Deriv.Mul

/-!
# Continuous effects: closed-form solubility

Proposition 4.1 of Howey, Long, and Fox, *VAL: Automatic Plan Validation,
Continuous Effects and Mixed Initiative Planning using PDDL*.

> Let `F = {f_1,...,f_n}` be a finite set of PNEs changing continuously on the
> interval `[0,T]` given by `df_i/dt = g_i(f_1,...,f_n)` for all `i`, where
> `g_i` is some function depending on `F`. The function `g_i` is restricted to
> addition, subtraction, multiplication and division on its terms, and division
> by a PNE in `F` is not permitted. If the rate of change of no PNE depends on
> itself (directly or indirectly) then the value of every PNE on `[0,T]` is
> given by a polynomial in `t`.
>
> Proof. Follows by induction on the dependency structure.

## Why this is stated but not proved

This is a genuine existence-of-closed-form-solution claim for a system of
(generally nonlinear, via `×`/`÷`) ordinary differential equations, and the
paper's own proof sketch -- one sentence, "induction on the dependency
structure" -- does not address the part that would make the induction work: a
solved PNE substituted into a sibling's rate expression through `÷` need not
stay polynomial after integration (`∫ 1/(1+t) dt` is not a polynomial in `t`),
so the "no division by a PNE" restriction alone is doing real, unstated work in
ruling that out, and exactly how is not spelled out in the source.

Formalizing this properly is its own project: it needs real ODE existence
theory (Mathlib has some of this, e.g. Picard-Lindelöf machinery, but wiring it
to a syntactic dependency-acyclicity argument for a mutually-referential system
is substantial). Rather than force a proof through, or substitute a weaker
discrete analogue and call it the same theorem, this module states Proposition
4.1 exactly and leaves it unproved -- an honest `Stated`/`Interfaced` claim, per
`~/POWL`'s own discipline: "Unproved repository claims are represented as
explicit structures or predicates … rather than axioms disguised as completed
theorems."

What *is* proved: the one-PNE, constant-rate instance (`g_0 = const c`), where
`f_0(t) = c·t + f_0(0)` is a genuine degree-1 polynomial solution -- real
content, not a stand-in for the general claim, and evidence the definitions
below are not vacuous.
-/

namespace VAL
namespace Continuous

variable {n : Nat}

/-- The syntax `g_i` is restricted to: addition, subtraction, multiplication,
and division "on its terms" -- an explicit expression type over the other
PNEs' current values, so the restriction is enforced by what can be
*constructed*, not merely asserted of an opaque function. -/
inductive RateExpr (n : Nat)
  | const (c : ℝ)
  | var (i : Fin n)
  | add (a b : RateExpr n)
  | sub (a b : RateExpr n)
  | mul (a b : RateExpr n)
  | div (a b : RateExpr n)

namespace RateExpr

/-- Evaluate a rate expression against a snapshot of every PNE's current
value. Noncomputable: `ℝ`'s field division is. This is only ever used inside
`Prop`s (`IsSolution`, `Proposition_4_1`) and small worked proofs, never
executed. -/
noncomputable def eval (v : Fin n → ℝ) : RateExpr n → ℝ
  | const c => c
  | var i => v i
  | add a b => a.eval v + b.eval v
  | sub a b => a.eval v - b.eval v
  | mul a b => a.eval v * b.eval v
  | div a b => a.eval v / b.eval v

/-- The PNE indices an expression reads. -/
def vars : RateExpr n → Finset (Fin n)
  | const _ => ∅
  | var i => {i}
  | add a b | sub a b | mul a b | div a b => a.vars ∪ b.vars

/-- The paper's restriction: "division by a PNE in `F` is not permitted".
Read as: no `div` subterm's divisor is *directly* a bare PNE reference (a
divisor built from a constant, or from a compound expression that is not
itself a single `var`, is not what the paper is excluding -- the concern is
literally dividing by a changing quantity). This is a specific reading of a
one-sentence restriction; recorded here rather than silently assumed. -/
def NoVarDivisor : RateExpr n → Prop
  | const _ | var _ => True
  | add a b | sub a b | mul a b => a.NoVarDivisor ∧ b.NoVarDivisor
  | div a b => a.NoVarDivisor ∧ b.NoVarDivisor ∧ ∀ i, b ≠ var i

end RateExpr

/-- PNE `i`'s rate directly reads PNE `j`'s value. -/
def DependsOn (rate : Fin n → RateExpr n) (i j : Fin n) : Prop :=
  j ∈ (rate i).vars

/-- "The rate of change of no PNE depends on itself (either directly or
indirectly)": no PNE index is reachable from itself via one or more
`DependsOn` steps. `Relation.TransGen` (strict, at-least-one-step transitive
closure) rather than `Relation.ReflTransGen`: reflexive closure would make
*every* index trivially "depend on itself" via zero steps, which is not what
the paper means. -/
def NoSelfDependency (rate : Fin n → RateExpr n) : Prop :=
  ∀ i, ¬ Relation.TransGen (DependsOn rate) i i

/-- A function is a polynomial on `[0,T]` when it agrees there with the
evaluation of some explicit, finite coefficient list -- `coeffs[k]` the
coefficient of `t^k`. Spelled out rather than reached for Mathlib's
`Polynomial` type, so "given by a polynomial in `t`" is exactly the paper's
phrase: an explicit finite closed form, checkable by direct computation. -/
def IsPolynomialOn (f : ℝ → ℝ) (T : ℝ) (coeffs : List ℝ) : Prop :=
  ∀ t, 0 ≤ t → t ≤ T →
    f t = ∑ k ∈ Finset.range coeffs.length, coeffs.getD k 0 * t ^ k

/-- A PNE system: `n` PNEs, each with a rate expression restricted per
`RateExpr.NoVarDivisor`, that does not depend on itself. Bundles exactly
Proposition 4.1's hypotheses on `g`; says nothing yet about a solution. -/
structure PNESystem (n : Nat) where
  rate : Fin n → RateExpr n
  noVarDivisor : ∀ i, (rate i).NoVarDivisor
  noSelfDependency : NoSelfDependency rate

/-- `f` solves `sys` on `[0,T]` from the given initial values: each `f i` is
differentiable at every `t ∈ [0,T]` with derivative `(sys.rate i).eval
(fun j => f j t)` -- exactly `df_i/dt = g_i(f_1,...,f_n)` -- and starts at
`initial i` at `t = 0`. -/
structure IsSolution (sys : PNESystem n) (T : ℝ) (f : Fin n → ℝ → ℝ)
    (initial : Fin n → ℝ) : Prop where
  hasDeriv : ∀ i t, 0 ≤ t → t ≤ T →
      HasDerivAt (f i) ((sys.rate i).eval (fun j => f j t)) t
  initial_eq : ∀ i, f i 0 = initial i

/-- Proposition 4.1. Under `sys`'s hypotheses, any solution on `[0,T]` is
polynomial in `t` on that interval -- for each PNE, *some* finite coefficient
list witnesses it.

**Not proved.** See the module docstring: the paper's one-line proof sketch
does not establish that division by a non-PNE divisor preserves polynomiality
under integration, and a faithful proof needs ODE existence/uniqueness theory
this pass does not build. Stated exactly, not weakened, not axiomatized. -/
def Proposition_4_1 (sys : PNESystem n) (T : ℝ) (_hT : 0 ≤ T) (f : Fin n → ℝ → ℝ)
    (initial : Fin n → ℝ) (_hsol : IsSolution sys T f initial) : Prop :=
  ∀ i, ∃ coeffs : List ℝ, IsPolynomialOn (f i) T coeffs

end Continuous

namespace Continuous.Examples

open Continuous

/-- The smallest non-vacuous instance: one PNE with a constant rate. Its rate
expression has no divisor at all (so `NoVarDivisor` holds vacuously) and,
having no `var` subterm, cannot depend on any PNE including itself (so
`NoSelfDependency` holds vacuously -- there is no `DependsOn` edge whatsoever
for a constant rate). -/
def constantRateSystem (c : ℝ) : PNESystem 1 where
  rate := fun _ => .const c
  noVarDivisor := fun _ => trivial
  noSelfDependency := by
    have hEmpty : ∀ i j : Fin 1, ¬ DependsOn (fun _ => RateExpr.const c) i j := by
      intro i j hd
      simp [DependsOn, RateExpr.vars] at hd
    intro i h
    cases h with
    | single hr => exact hEmpty _ _ hr
    | tail _ hr => exact hEmpty _ _ hr

/-- `f(t) = c·t + f₀` solves the constant-rate system, for any starting value
`f₀`. -/
def constantRateSolution (c f0 : ℝ) : Fin 1 → ℝ → ℝ := fun _ t => c * t + f0

theorem constantRateSolution_isSolution (c f0 T : ℝ) :
    IsSolution (constantRateSystem c) T (constantRateSolution c f0)
      (fun _ => f0) where
  hasDeriv i t _h0 _hT := by
    have hderiv : HasDerivAt (fun t : ℝ => c * t + f0) c t := by
      have h1 : HasDerivAt (fun t : ℝ => c * t) c t := by
        have := (hasDerivAt_id t).const_mul c
        simpa using this
      simpa using h1.add_const f0
    have hrate : (constantRateSystem c).rate i = RateExpr.const c := rfl
    show HasDerivAt (constantRateSolution c f0 i)
      ((constantRateSystem c).rate i |>.eval (fun j => constantRateSolution c f0 j t)) t
    rw [hrate]
    exact hderiv
  initial_eq _i := by simp [constantRateSolution]

/-- The proved corroborating instance: this concrete constant-rate PNE is a
genuine degree-1 polynomial, `c·t + f₀` (coefficients `[f₀, c]`), a real
instance of what the general (unproved) `Proposition_4_1` claims. -/
theorem constantRateSolution_isPolynomial (c f0 T : ℝ) :
    IsPolynomialOn (fun t => constantRateSolution c f0 0 t) T [f0, c] := by
  intro t _ht0 _htT
  simp only [constantRateSolution, List.length_cons, List.length_nil,
    Finset.sum_range_succ, Finset.sum_range_zero, List.getD_cons_zero,
    List.getD_cons_succ, zero_add]
  ring

end Continuous.Examples
end VAL
