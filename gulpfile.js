var gulp = require('gulp')
    , connect = require('gulp-connect')
    , bowerFiles = require('main-bower-files')
    , del = require('del')
    , merge = require('merge-stream')
    , inject = require('gulp-inject')
    , filter = require('gulp-filter')
    , open = require('gulp-open')
    , zip = require('gulp-zip')
    ;

var bowerPkg = require("./bower.json");
var packageName = bowerPkg.name + "-" + bowerPkg.version;

/**
 * Server and watch the development files.
 */

gulp.task("connect", function () {
    return connect.server({
        root: ["app"],
        port: 9090,
        livereload: true
    })
});

gulp.task("watch", ["connect"], function () {
    gulp.watch(["app/**/*"], function () {
        gulp.src("app/**/*").pipe(connect.reload());
    });
    gulp.src("app/index.html")
        .pipe(open("", {url: "http://localhost:9090"}));
});

/**
 * Serve and watch the build directory.
 */

gulp.task("connect:build", function () {
    return connect.server({
        root: ["build"],
        port: 9090,
        livereload: true
    })
});

/**
 * Building.
 */

gulp.task("watch:build", ["connect:build"], function () {
    gulp.watch(["app/**/*"], ["build", function () {
        gulp.src("app/**/*").pipe(connect.reload());
    }]);
    gulp.src("app/index.html")
        .pipe(open("", {url: "http://localhost:9090"}));
});

var copyApp = function () {
    return gulp.src(["app/**/*", "!app/bower_components/**/*"])
        .pipe(gulp.dest("./build/"));
};

var copyBowerComponents = function () {
    return gulp.src(bowerFiles(), {base: "./app/"})
        .pipe(gulp.dest("./build/"));
};

gulp.task("build", ["clean"], function () {
    return merge(copyApp(), copyBowerComponents());
});

/**
 * Injects Bower components into the index.html.
 * - Run after adding or removing Bower dependencies.
 */
gulp.task("inject", function () {
    var bowerComponents = bowerFiles();
    var sources = gulp.src(bowerComponents, {read: false})
        .pipe(filter(['*', '!**/json3.js', '!**/es5-shim.js']));
    gulp.src("./app/index.html")
        .pipe(inject(sources, {relative: true}))
        .pipe(gulp.dest("./app/"));
});

/**
 * Packaging.
 */

gulp.task("package:gather", ["build"], function () {
    return gulp.src("build/**/*")
        .pipe(gulp.dest("evebox-" + bowerPkg.version));
});

gulp.task("package:zip", ["package:gather"], function () {
    return gulp.src(packageName + "/**/*")
        .pipe(zip(packageName + ".zip"))
        .pipe(gulp.dest("."));
});

gulp.task("package", ["package:zip"]);

/**
 * Cleaning.
 */

/* Remove the build directory. */
gulp.task("clean", function (cb) {
    del(["build", "evebox-*"], cb);
});
