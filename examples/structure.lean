-- Structures: definition, constructor, and field access

structure Point where
  x : Nat
  y : Nat

def describe (p : Point) : String :=
  s!"Point({px}, {py})"
  where px := Point.x p
  where py := Point.y p

def main : IO Unit := do
  let origin := Point.mk 0 0
  let p := Point.mk 3 7
  IO.println (describe origin)
  IO.println (describe p)
