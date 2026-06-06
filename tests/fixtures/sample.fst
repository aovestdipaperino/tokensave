/// Comprehensive F* fixture for the tokensave extractor.
///
/// This file is a *syntactic* fixture — it exercises every construct the
/// extractor understands and is deliberately not meant to typecheck.
module Sample.Comprehensive

open FStar.Mul
include FStar.Tactics.V2
module L = FStar.List.Tot.Base

#set-options "--max_fuel 1 --max_ifuel 0 --z3rlimit 20"

(* A block comment with a ) stray ( paren and a "quote" — must be ignored. *)

/// A record type.
type point = { x : int; y : int }

/// An inductive type with several constructor shapes.
type shape =
  | Circle : radius:nat -> shape
  | Rect of nat & nat
  | Origin

/// Mutually recursive inductive types.
type tree =
  | Leaf
  | Branch of forest
and forest =
  | Nil
  | Cons of tree & forest

/// A parameterised type abbreviation.
type predicate (a:Type) = a -> bool

/// A typeclass: one [@@@no_method] field and one published method.
class addable (a:Type) = {
  [@@@no_method] zero : a;
  add : a -> a -> a;
}

/// An instance of the typeclass.
instance addable_int : addable int = {
  zero = 0;
  add = (fun x y -> x + y);
}

/// A definition that requires a resolved typeclass instance.
let sum_with (#a:Type) {| d : addable a |} (x y : a) : a = d.add x y

/// Record value construction via let (not a record *type*).
let origin : point = { x = 0; y = 0 }

/// A val declaration with no body.
val is_zero : int -> bool

/// A refinement-typed val.
val abs : x:int -> y:int{y >= 0 /\ (y = x \/ y = -x)}

/// A plain definition.
let negate (x:int) : int = -x

/// An unfold definition.
unfold
let twice (n:int) : int = n + n

/// An irreducible definition.
irreducible
let magic : int = 42

/// An inline_for_extraction, private definition.
inline_for_extraction
private
let bump (n:int) : int = n + 1

/// A lemma with requires / ensures / decreases.
let rec fact_pos (n:nat) : Lemma (requires n >= 0) (ensures factorial n >= 1) (decreases n) =
  if n = 0 then () else fact_pos (n - 1)

/// A Pure definition with requires / ensures / decreases.
let rec divmod (x:nat) (y:pos) : Pure nat (requires x >= 0) (ensures fun r -> r >= 0) (decreases x) =
  if x < y then 0 else 1 + divmod (x - y) y

/// A Tot definition with a decreases clause.
let rec countdown (n:nat) : Tot nat (decreases n) =
  if n = 0 then 0 else countdown (n - 1)

/// A symbolic operator.
let ( +^ ) (a b : int) : int = a + b

/// A monadic binding operator.
let ( let? ) (x : option 'a) (f : 'a -> option 'b) : option 'b =
  match x with
  | Some v -> f v
  | None -> None

/// Mutually recursive functions.
let rec even (n:nat) : bool = if n = 0 then true else odd (n - 1)
and odd (n:nat) : bool = if n = 0 then false else even (n - 1)

/// An exception.
exception Out_of_bounds

/// An effect abbreviation.
effect Id (a:Type) = Pure a (requires True) (ensures (fun _ -> True))

/// A string literal carrying braces and parens must not be read as a record.
let banner : string = "menu { open ( close ) }"

/// A proof body using a local let, introduce/eliminate and calc — no phantom nodes.
let comm_proof (x y : int) : Lemma (x + y == y + x) =
  let z = x + y in
  introduce exists (w:int). w == z
  with z
  and ();
  calc (==) {
    x + y;
    == { () }
    y + x;
  }

/// A lemma whose spec nests a local `let ... in` and an inline comment with stray parens.
let spec_let (x : int) : Lemma (* note: ) ( mismatched *) (requires (let p = x > 0 in p)) (ensures x >= 0) =
  ()

#push-options "--z3rlimit 50 --split_queries no"
/// Verified under pushed options.
let heavy (a : int) : Lemma (requires a > 0) (ensures a * a > 0) = ()
#pop-options

/// An assume val axiom (the `val` keyword wins; this is a Function).
assume val excluded_middle (p:Type0) : Lemma (p \/ ~p)

/// A monadic let in a body (uses the let? operator defined above).
let try_add (a b : option int) : option int =
  let? x = a in
  let? y = b in
  Some (x + y)
