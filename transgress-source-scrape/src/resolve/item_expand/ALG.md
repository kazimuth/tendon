{ $($root:ident [$($child:ident)+])+ $($after:lit)+} => { $($root $after [$(\$child)+])+ }

A [b c d] E [f g h] 5 10
A 5 [b c d] E 10 [f g h]

repetition: - collect all variables expanded this level - expand out to those variables
