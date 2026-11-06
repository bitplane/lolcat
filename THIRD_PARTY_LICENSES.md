# Third-party licences

The Rust code is a port of [lolcat](https://github.com/busyloop/lolcat), which
is covered by [LICENSE](LICENSE). Two of its Ruby dependencies were also ported
in part, since matching their output byte for byte means following their logic.

## Paint

The RGB to 256-colour conversion in `src/color.rs` is ported from
[Paint](https://github.com/janlelis/paint), copyright (c) 2011-2020 Jan Lelis.

## Optimist

The argument handling in `src/main.rs` follows the behaviour of
[Optimist](https://github.com/ManageIQ/optimist), copyright (c) 2008-2014
William Morgan and (c) 2014 Red Hat, Inc.

## MIT License

Both are distributed under these terms:

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
