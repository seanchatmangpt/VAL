import Mathlib.Data.List.Sort
import Mathlib.Data.Rat.Defs

/-!
# Acts and semi-simple plans

Definitions 3.1, 3.2, and 3.3 of Howey, Long, and Fox, *VAL: Automatic Plan
Validation, Continuous Effects and Mixed Initiative Planning using PDDL*.

The paper introduces semi-simple plans because a plan with continuous effects
cannot be expressed as a simple plan: the value of a primitive numerical
expression (PNE) may be needed at arbitrary points of an interval over which it
is still changing. A semi-simple plan adds enough structure -- act *types* -- to
say when invariants are checked and when continuous updates are applied,
without committing to a continuous semantics at the syntactic level. That
semantics is the concern of `VAL.Execution` and `VAL.Continuous`; this module is
purely about the paper's syntax and its one nontrivial well-formedness clause
(Definition 3.3's positivity and ordering conditions).
-/

namespace VAL

/-- Definition 3.1 (Act), the label part. An act type is one of three values.

The constructor order is not cosmetic: Definition 3.3 orders act sequences
lexicographically by time and *then by act type*, stating that order as
"invariant, continuous update, regular". `ActType.rank` below is exactly that
order, and it is what makes `SemiSimplePlan.actSequence`'s ordering well
defined. -/
inductive ActType
  | invariant
  | continuousUpdate
  | regular
  deriving DecidableEq, Repr, Inhabited

namespace ActType

/-- The paper's ordering on act types: invariant, then continuous update, then
regular. At a single time point the invariants are checked first, the
continuous effects applied next, and the instantaneous (regular) effects
last -- matching Definition 3.4's per-act-type dispatch. -/
def rank : ActType → Nat
  | invariant => 0
  | continuousUpdate => 1
  | regular => 2

theorem rank_invariant_lt_continuousUpdate :
    rank invariant < rank continuousUpdate := by decide

theorem rank_continuousUpdate_lt_regular :
    rank continuousUpdate < rank regular := by decide

/-- `rank` separates the three act types, so ordering by it loses nothing. -/
theorem rank_injective {a b : ActType} (h : rank a = rank b) : a = b := by
  cases a <;> cases b <;> simp_all [rank]

end ActType

/-- Definition 3.1 (Act). A happening labelled with an act type. The happening
itself is a time, matching the paper's usage: acts are what occur at the time
points of a plan. -/
structure Act where
  time : Rat
  actType : ActType
  deriving DecidableEq, Repr

/-- Definition 3.2 (Semi-Simple Plan), one element. A timed simple action is the
3-tuple `(t, A, a)`: a rational-valued time, an act type, and an action name.
The action name is left as a type parameter because nothing in Definitions
3.1-3.5 inspects it -- only a concrete domain semantics (`VAL.Execution`) does. -/
structure TimedSimpleAction (Name : Type u) where
  time : Rat
  actType : ActType
  action : Name
  deriving Repr

namespace TimedSimpleAction

variable {Name : Type u}

/-- The act of a timed simple action: its time and act type, forgetting which
action occurred. This is exactly the projection Definition 3.3 takes to build
an act sequence. -/
def act (tsa : TimedSimpleAction Name) : Act :=
  { time := tsa.time, actType := tsa.actType }

end TimedSimpleAction

/-- Definition 3.2 (Semi-Simple Plan). A finite collection of timed simple
actions. Represented as a `List` rather than a `Finset`: the paper says
"collection", and two distinct durative actions can contribute identically
timed and identically typed acts (e.g. two regular acts firing at the same
instant from different actions), so multiplicity is meaningful and a `Finset`
would silently collapse it. -/
structure SemiSimplePlan (Name : Type u) where
  actions : List (TimedSimpleAction Name)
  deriving Repr

namespace SemiSimplePlan

variable {Name : Type u}

/-- Definition 3.3's positivity side condition: "For all `i`, `t_i > 0`." -/
def TimesPositive (plan : SemiSimplePlan Name) : Prop :=
  ∀ tsa ∈ plan.actions, 0 < tsa.time

instance (plan : SemiSimplePlan Name) : Decidable (plan.TimesPositive) := by
  unfold TimesPositive; infer_instance

/-- The lexicographic order of Definition 3.3: by time, then by act type. -/
def actLe (left right : Act) : Bool :=
  if left.time = right.time then
    left.actType.rank ≤ right.actType.rank
  else
    decide (left.time < right.time)

/-- The plan's timed simple actions, sorted by Definition 3.3's lexicographic
order on their underlying acts. Kept as the primitive sort (rather than sorting
the already-projected acts and separately re-deriving this) because
`VAL.Execution`'s Definition 3.4 needs the action name at each step, not just
its time and type -- Definition 3.3 fixes an order on acts, and this is that
same order applied to the richer records the acts came from. -/
def orderedActions (plan : SemiSimplePlan Name) : List (TimedSimpleAction Name) :=
  plan.actions.mergeSort (fun a b => actLe a.act b.act)

theorem orderedActions_perm (plan : SemiSimplePlan Name) :
    (plan.orderedActions).Perm plan.actions :=
  List.mergeSort_perm _ _

/-- Definition 3.3 (Act Sequence for a Semi-Simple Plan). The sequence of time
and act-type pairs appearing in the plan's timed simple actions, ordered
lexicographically by time and then by act type. Defined as the projection of
`orderedActions`, so its ordering and content are inherited rather than
re-derived by a second, independent sort. -/
def actSequence (plan : SemiSimplePlan Name) : List Act :=
  plan.orderedActions.map TimedSimpleAction.act

/-- The act sequence contains exactly the acts of the plan's actions, with
multiplicity: sorting is a permutation, so Definition 3.3 discards nothing. -/
theorem actSequence_perm (plan : SemiSimplePlan Name) :
    (plan.actSequence).Perm (plan.actions.map TimedSimpleAction.act) :=
  (plan.orderedActions_perm).map _

/-- Hence the act sequence has one entry per timed simple action. -/
theorem length_actSequence (plan : SemiSimplePlan Name) :
    plan.actSequence.length = plan.actions.length := by
  have h := (plan.actSequence_perm).length_eq
  simpa using h

end SemiSimplePlan
end VAL
