import VAL.Act

/-!
# Executability and validity

Definitions 3.4 and 3.5 of Howey, Long, and Fox, *VAL: Automatic Plan
Validation, Continuous Effects and Mixed Initiative Planning using PDDL*.

The paper's Definition 3.4 dispatches on act type: a regular act checks and
applies PDDL preconditions/effects; an invariant act checks a condition over the
interval since the preceding regular act; a continuous-update act applies the
continuous effect accumulated over that same interval. Concretely evaluating
any of these requires a full PDDL domain semantics -- fluent lookup, numeric
expression evaluation, the actual solution of the continuous-effect ODE
`VAL.Continuous` states but does not solve -- which is its own large
undertaking and out of scope here.

What *is* in scope, and is real content rather than an assumption: given any
semantics satisfying this shape (an `ActSemantics`), the recursive
per-act-type dispatch that Definition 3.4 describes, the trace it produces, and
Definition 3.5's validity on top of it are all fully and concretely defined, and
the paper's own remark -- "an executable plan produces a unique trace" -- is
proved rather than assumed, because `runFrom` below is a genuine (deterministic,
partial) function, not a relation a plan could satisfy in more than one way.
-/

namespace VAL

/-- The state carried between acts: the current state, plus the state and time
of the most recently executed regular act. Definition 3.4 checks invariant and
continuous-update acts "over the interval between the preceding regular act and
`t_i`", so that interval's two endpoints must be threaded through -- they are
not otherwise recoverable from `current` alone. -/
structure ExecState (State : Type u) where
  current : State
  regularState : State
  regularTime : Rat
  deriving Repr

namespace ExecState

/-- The state before any act has executed. The paper does not fix a "time
zero" for the interval preceding the first regular act; `0` is a modelling
choice, sound because Definition 3.3's `TimesPositive` guarantees every real
act time is strictly greater than it, so no real interval is ever degenerate or
negative as a result. -/
def init (state : State) : ExecState State :=
  { current := state, regularState := state, regularTime := 0 }

end ExecState

/-- An interpretation of act semantics, parameterized over a state type and an
action-name type. This is the honest carrier for everything Definition 3.4
presupposes but does not itself define -- PDDL fluent evaluation, numeric
effect application, and continuous-effect integration. A value of this
structure is a hypothesis, not a proof: nothing here asserts any concrete
instance is faithful to PDDL, only that *given* one, executability and validity
are exactly Definitions 3.4 and 3.5.

Boolean-valued rather than `Prop`-valued so `step` below is a total,
`Decidable`-free function -- real PDDL condition evaluation is decidable, and
this avoids threading `DecidablePred` instances through every downstream
definition for no semantic gain. -/
structure ActSemantics (State Name : Type u) where
  /-- Definition 3.4, regular case: "the preconditions of `Act_i` must hold in
  `S_i`". -/
  regularPrecondition : Name → State → Bool
  /-- Definition 3.4, regular case: "`S_{i+1}` is the result of removing delete
  effects, adding add effects and applying numeric effects". -/
  regularEffect : Name → State → State
  /-- Definition 3.4, invariant case: "the conditions of `Act_i` must hold over
  the interval between the preceding regular act and `t_i`", given the state
  and time of that preceding regular act and the current state and time. -/
  invariantHolds : Name → (regularState current : State) →
      (regularTime currentTime : Rat) → Bool
  /-- Definition 3.4, continuous-update case: "the effects of `Act_i` are
  applied at time `t_i` for the continuous effects over the interval between
  the preceding regular act and `t_i`". -/
  continuousEffect : Name → (regularState current : State) →
      (regularTime currentTime : Rat) → State

variable {State Name : Type u}

/-- One step of Definition 3.4's dispatch: execute a single timed simple
action against the current `ExecState`, per its act type. Returns `none`
exactly when the paper says the plan fails to be executable at this point: a
regular act whose precondition does not hold, or an invariant act whose
condition does not hold over the interval. An invariant act that holds changes
nothing (it is a check, not an effect); a continuous-update act always
succeeds and updates `current`, since Definition 3.4 attaches no precondition
to it. -/
def ActSemantics.step (sem : ActSemantics State Name) (es : ExecState State)
    (tsa : TimedSimpleAction Name) : Option (ExecState State) :=
  match tsa.actType with
  | .regular =>
      if sem.regularPrecondition tsa.action es.current then
        let next := sem.regularEffect tsa.action es.current
        some { current := next, regularState := next, regularTime := tsa.time }
      else none
  | .invariant =>
      if sem.invariantHolds tsa.action es.regularState es.current
          es.regularTime tsa.time then
        some es
      else none
  | .continuousUpdate =>
      some { es with
        current :=
          sem.continuousEffect tsa.action es.regularState es.current
            es.regularTime tsa.time }

/-- The trace produced by running a semantics over an ordered list of timed
simple actions from a starting `ExecState`, one `ExecState` per act plus the
initial one -- `none` if any step fails. A genuine recursive function, not a
relation, which is exactly what makes uniqueness of the resulting trace free
(see `runFrom_unique` below) rather than a fact requiring its own proof. -/
def runFrom (sem : ActSemantics State Name) (es : ExecState State) :
    List (TimedSimpleAction Name) → Option (List (ExecState State))
  | [] => some [es]
  | tsa :: rest =>
      match sem.step es tsa with
      | none => none
      | some es' =>
          match runFrom sem es' rest with
          | none => none
          | some restTrace => some (es :: restTrace)

/-- Definition 3.4 (Executability of a Semi-Simple Plan), and simultaneously
its trace. `SSP` is executable (under `sem`, from `initial`) exactly when
`runFrom` returns `some`, and that value is the trace Definition 3.4 names:
"the state sequence `{S_i}_{i=0...k+1}`". Executability and trace-existence
are the same fact by construction, matching the paper's own phrasing ("an
executable plan produces ... the trace of SSP"). -/
def Trace (sem : ActSemantics State Name) (plan : SemiSimplePlan Name)
    (initial : State) : Option (List (ExecState State)) :=
  runFrom sem (ExecState.init initial) plan.orderedActions

/-- "Note that an executable plan produces a unique trace." Proved, not
assumed: `Trace` is `Option.map`-free application of the deterministic function
`runFrom`, so any two evaluations of `Trace sem plan initial` are definitionally
the same value. There is no second execution for a `Trace` to disagree with. -/
theorem trace_unique (sem : ActSemantics State Name) (plan : SemiSimplePlan Name)
    (initial : State) (t₁ t₂ : List (ExecState State))
    (h₁ : Trace sem plan initial = some t₁)
    (h₂ : Trace sem plan initial = some t₂) : t₁ = t₂ := by
  rw [h₁] at h₂; exact (Option.some.injEq ..).mp h₂

/-- `runFrom` never returns the empty list: the initial `ExecState` is
prepended at every step, including the base case. This is what lets
`finalState` below take the last entry without needing `State` to be
`Inhabited`. -/
theorem runFrom_ne_nil (sem : ActSemantics State Name) (es : ExecState State)
    (actions : List (TimedSimpleAction Name)) {trace : List (ExecState State)}
    (h : runFrom sem es actions = some trace) : trace ≠ [] := by
  induction actions generalizing es trace with
  | nil =>
    simp only [runFrom, Option.some.injEq] at h
    simp [← h]
  | cons tsa rest _ih =>
    unfold runFrom at h
    split at h
    · exact absurd h (by simp)
    · split at h
      · exact absurd h (by simp)
      · exact (Option.some.injEq ..).mp h ▸ List.cons_ne_nil _ _

/-- The final state of a trace: the last entry's `current` state, i.e. the
state after every act in the sequence has been applied. Definition 3.4 calls
this "the final state produced by SSP". -/
def finalState (sem : ActSemantics State Name) (plan : SemiSimplePlan Name)
    (initial : State) : Option State :=
  match h : Trace sem plan initial with
  | none => none
  | some trace => some (trace.getLast (runFrom_ne_nil sem _ _ h)).current

/-- Definition 3.5 (Validity of a Semi-Simple Plan). Executable, and the final
state satisfies the goal. `Goal` is left abstract -- a predicate on `State` --
for the same reason `ActSemantics` leaves precondition/effect evaluation
abstract: Definition 3.5 does not itself specify what "the goal specification
is satisfied" means at the level of PDDL syntax, only that it is a predicate on
the final state. -/
def ValidSemiSimplePlan (sem : ActSemantics State Name)
    (plan : SemiSimplePlan Name) (initial : State) (Goal : State → Prop) : Prop :=
  ∃ final, finalState sem plan initial = some final ∧ Goal final

/-- Definition 3.5's second sentence: "A plan, `P` is valid if
`semi-simplify(P)` is valid." `semiSimplify` -- the paper's mapping from a
general plan (with durative actions) to a semi-simple plan (end points to
regular acts, invariant checks to invariant acts, continuous effects to
continuous-update acts, described in the paragraph following Definition 3.3) --
is not formalized here: it requires a concrete plan/durative-action syntax this
module does not fix. `Plan` and `semiSimplify` are therefore left as
parameters, so `ValidPlan` states exactly what the paper states without
inventing the missing piece. -/
def ValidPlan {Plan : Type u} (semiSimplify : Plan → SemiSimplePlan Name)
    (sem : ActSemantics State Name) (initial : State) (Goal : State → Prop)
    (plan : Plan) : Prop :=
  ValidSemiSimplePlan sem (semiSimplify plan) initial Goal

end VAL
