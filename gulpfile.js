var gulp          = require('gulp')
    , connect     = require('gulp-connect')
    , bowerFiles  = require('main-bower-files')
    , del         = require('del')
    , merge       = require('merge-stream')
    , inject      = require('gulp-inject')
    , filter      = require('gulp-filter')
    , open        = require('gulp-open')
    , zip         = require('gulp-zip')
    , ngAnnotate  = require('gulp-ng-annotate')
    , runSequence = require('run-sequence')
    , less        = require('gulp-less')
    ;

var bowerPkg = require("./bower.json");
var packageName = bowerPkg.name + "-" + bowerPkg.version;

var port = 9090;

gulp.task("serve", ["inject", "build:less"], function () {

    connect.server({
        root: "app",
        port: port,
        livereload: true
    });

    var appSources = "app/**/*";

    gulp.watch(appSources, ["inject"]);
    gulp.watch("app/styles/*.less", ["build:less"]);
    gulp.watch(appSources, function () {
        gulp.src(appSources).pipe(connect.reload());
    });

    // Open a browser.
    gulp.src("app/index.html")
        .pipe(open("", {url: "http://localhost:9090"}));
});

/**
 * Building.
 */

gulp.task("watch:build", ["build"], function () {

    connect.server({
        root: "build",
        port: port,
        livereload: true
    });

    gulp.watch(["app/**/*"], ["build", function () {
        gulp.src("app/**/*").pipe(connect.reload());
    }]);
    gulp.src("app/index.html")
        .pipe(open("", {url: "http://localhost:9090"}));
});

/**
 * Simply copies over all required bower components.
 */
gulp.task("build:bower_components", function () {
    return gulp.src(bowerFiles(), {base: "./app/"})
        .pipe(gulp.dest("./build/"));
});

/**
 * Build and copy the application Javascript.
 */
gulp.task("build:app:js", function () {
    return gulp.src(["app/scripts/**/*.js"], {base: "./app/"})
        .pipe(ngAnnotate())
        .pipe(gulp.dest("./build/"));
});

/**
 * Build and copy the application html, styles and other files.
 */
gulp.task("build:app", ["build:less"], function () {
    var sources = [
        "app/*",
        "app/styles/*",
        "app/templates/*"
    ];
    return gulp.src(sources, {base: "./app/"})
        .pipe(gulp.dest("./build/"));
});

gulp.task("build:less", function() {
   gulp.src("app/styles/app.less")
       .pipe(less())
       .pipe(gulp.dest("app/styles"));
});

gulp.task("build", function (cb) {
    runSequence("clean", "inject",
        ["build:bower_components", "build:app:js", "build:app"], cb);
});

/**
 * Inject Bower components.
 */
gulp.task("inject:bower", function () {
    var bowerComponents = bowerFiles();
    var sources = gulp.src(bowerComponents, {read: false})
        .pipe(filter(['*', '!**/json3.js', '!**/es5-shim.js']));
    return gulp.src("./app/index.html")
        .pipe(inject(sources, {relative: true}))
        .pipe(gulp.dest("./app/"));
});

/**
 * Inject application sources.
 */
gulp.task("inject:app", function () {
    var sources = gulp.src(["app/scripts/app.js", "app/scripts/*.js"],
        {read: false});
    return gulp.src("./app/index.html")
        .pipe(inject(sources, {relative: true, name: "app"}))
        .pipe(gulp.dest("./app/"));
});

gulp.task("inject", function (cb) {
    runSequence("inject:bower", "inject:app", cb);
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
