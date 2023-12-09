# How to Use

You can use `regex` as its own library with some tweaks, though after some breaking soundness changes it's slower than
the de facto standard library on most test cases, so there's not much point. I will remove huge recursive branches so 
that we don't run rubbish and slow!

## Creating a Regex

```rs
use regex::buld_guide::Re;

let build_guide = "some text".seq("some more text").seq('c').alt('a'.alt('b')).star();
let r = Regex::from(&build_guide);
```

In this case teh regex is equivalent to

```
(some textsome more textc|(a|b))*
```

## Matching a String

```rs
let r: Regex = ...;
let result: bool = r.is_match("ababababab");
```