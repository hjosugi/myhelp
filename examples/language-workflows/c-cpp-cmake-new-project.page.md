# C and C++ CMake project

> Configure, build, and test a portable C or C++ project with CMake.
> Official documentation: <https://cmake.org/cmake/help/latest/guide/tutorial/index.html>.

- Keep `CMakeLists.txt` and sources in the source tree, then configure an out-of-source debug build:

`cmake -S . -B build -DCMAKE_BUILD_TYPE=Debug`

- Build in parallel without depending on a particular native generator:

`cmake --build build --parallel`

- Run tests registered with CTest and show failures:

`ctest --test-dir build --output-on-failure`

- Build a specific target:

`cmake --build build --target {{target}}`

- Install using the rules declared by the project:

`cmake --install build --prefix {{prefix}}`
