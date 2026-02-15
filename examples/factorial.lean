def factorial (n : Nat) : Nat :=
  if n == 0 then 1 else n * factorial (n - 1)

def main : IO Unit := do
  IO.println (toString (factorial 5))
