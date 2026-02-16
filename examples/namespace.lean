-- Namespaces and open

namespace Math
def square (x : Nat) : Nat := x * x
def cube (x : Nat) : Nat := x * x * x
end Math

open Math

def main : IO Unit := do
  IO.println s!"5^2 = {square 5}"
  IO.println s!"3^3 = {cube 3}"
