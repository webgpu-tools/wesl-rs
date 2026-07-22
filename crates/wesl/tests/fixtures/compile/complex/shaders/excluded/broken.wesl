// This file is deliberately NOT valid WESL. It lives under a directory matched
// by the `exclude` glob in wesl.toml, so the file scanner must skip it.
// If compilation fails because of this file, the exclude field is broken.

import $$$ not a valid import !!!

fn { this is not a function
