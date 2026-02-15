inductive Color where
  | red
  | green
  | blue

def colorName (c : Color) : String :=
  match c with
  | Color.red => "Red"
  | Color.green => "Green"
  | Color.blue => "Blue"

def main : IO Unit := do
  IO.println (colorName Color.red)
  IO.println (colorName Color.blue)
