---
title: Callbacks
---

# Callbacks
When defining your plan, you have the flexibility to override the default behavior of Habitat in each part of the package building stage through a series of callbacks. To define a callback, simply create a shell function of the same name in your `plan.sh` file and then write your script. If you do not want to use the default callback behavior, you must override the callback and `return 0` in the function definition.

These callbacks are listed in the order that they are called by the package build script.

**do_begin()**
: There is an empty default implementation of this callback. You can use it to execute any arbitrary commands before anything else happens. Note that at this phase of the build, no dependencies are resolved, the `$PATH` and environment is not set, and no external source has been downloaded. For a phase that is more completely set up, see the `do_before()` phase.

**do_before()**
: There is an empty default implementation of this callback. At this phase, the origin key has been checked for, all package dependencies have been resolved and downloaded, and the `$PATH` and environment are set.

**do_download()**
: The default implementation is that the software specified in `$pkg_source` is downloaded, checksum-verified, and placed in `$HAB_CACHE_SRC_PATH/$pkg_filename`, which resolves to a path like `/hab/cache/src/filename.tar.gz`. You should override this behavior if you need to change how your binary source is downloaded, if you are not downloading any source code at all, or if you are cloning from git. If you do clone a repo from git, you must override **do_verify()** to return 0.

**do_verify()**
: The default implementation tries to verify the checksum specified in the plan against the computed checksum after downloading the source tarball to disk. If the specified checksum doesn't match the computed checksum, then an error and a message specifying the mismatch will be printed to stderr. You should not need to override this behavior unless your package does not download any files.

**do_clean()**
: The default implementation removes the `HAB_CACHE_SRC_PATH/$pkg_dirname` folder in case there was a previously-built version of your package installed on disk. This ensures you start with a clean build environment.

**do_unpack()**
: The default implementation extracts your tarball source file into `HAB_CACHE_SRC_PATH`. The supported archive extensions are: .tar, .tar.bz2, .tar.gz, .tar.xz, .rar, .zip, .Z, .7z. If the file archive could not be found or has an unsupported extension, then a message will be printed to stderr with additional information.

**do_prepare()**
: There is no default implementation of this callback. At this point in the build process, the tarball source has been downloaded, unpacked, and the build environment variables have been set, so you can use this callback to perform any actions before the package starts building, such as exporting variables, adding symlinks, and so on.

**do_build()**
: The default implementation is to update the prefix path for the configure script to use `$pkg_prefix` and then run `make` to compile the downloaded source. This means the script in the default implementation does `./configure --prefix=$pkg_prefix && make`. You should override this behavior if you have additional configuration changes to make or other software to build and install as part of building your package.

**do_check()**
: The default implementation runs nothing during post-compile.  An example of a command you might use in this callback is `make test`. To use this callback, two conditions must be true. A) `do_check()` function has been declared, B) `DO_CHECK` environment variable exists and set to true, `env DO_CHECK=true`.

**do_install()**
: The default implementation is to run `make install` on the source files and place the compiled binaries or libraries in `HAB_CACHE_SRC_PATH/$pkg_dirname`, which resolves to a path like `/hab/cache/src/packagename-version/`. It uses this location because of **do_build()** using the `--prefix` option when calling the configure script. You should override this behavior if you need to perform custom installation steps, such as copying files from `HAB_CACHE_SRC_PATH` to specific directories in your package, or installing pre-built binaries into your package.

**do_strip()**
: The default implementation is to strip any binaries in `$pkg_prefix` of their debugging symbols. You should override this behavior if you want to change how the binaries are stripped, which additional binaries located in subdirectories might also need to be stripped, or whether you do not want the binaries stripped at all.

**do_after()**
: There is an empty default implementation of this callback. At this phase, the package has been built, installed, stripped, but before the package metadata is written and the artifact is created and signed.

**do_end()**
: There is an empty default implementation of this callback. This is called after the package artifact has been created. You can use this callback to remove any temporary files or perform other post-build clean-up actions.
