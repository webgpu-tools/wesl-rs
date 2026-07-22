// module `package::prelude`: a pure re-export module.
// `public import` re-exports the imported items under this module's path, so
// importers can write `import package::prelude::{pdot2, x2, dbl};` or, thanks
// to the module attribute below, `import package::prelude::*;`.

// re-exports with renaming; the originals must be `public` (a re-export
// cannot widen visibility).
// TODO: visibility
// public import package::util::{dot2 as pdot2, cross2 as x2};
// public import package::util::ops::double as dbl;
@public import package::util::{dot2 as pdot2, cross2 as x2};
@public import package::util::ops::double as dbl;

// TODO: wildcards
// @!wildcardable;
