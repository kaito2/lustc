def add (x : Nat) (y : Nat) : Nat := x + y

def mul (x : Nat) (y : Nat) : Nat := x * y

def main : IO Unit := do
  IO.println (toString (add 3 4))
  IO.println (toString (mul 5 6))
