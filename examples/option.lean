-- Option type: Some/None with pattern matching

def safeDiv (x : Nat) (y : Nat) : Option Nat :=
  if y == 0 then Option.none
  else Option.some (x / y)

def showResult (o : Option Nat) : String :=
  match o with
  | Option.none => "undefined"
  | Option.some v => s!"result: {v}"

def main : IO Unit := do
  IO.println (showResult (safeDiv 10 3))
  IO.println (showResult (safeDiv 10 0))
