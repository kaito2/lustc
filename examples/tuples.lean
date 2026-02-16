-- Tuples: creation, field access, and pattern matching

def swap (p : Nat × Nat) : Nat × Nat :=
  match p with
  | (a, b) => (b, a)

def fst (p : Nat × Nat) : Nat := p.1
def snd (p : Nat × Nat) : Nat := p.2

def main : IO Unit := do
  let p := (10, 20)
  IO.println s!"original: ({fst p}, {snd p})"
  let q := swap p
  IO.println s!"swapped: ({fst q}, {snd q})"
