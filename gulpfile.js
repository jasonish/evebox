var gulp = require('gulp')
    , connect = require('gulp-connect')
    , bowerFiles = require('main-bower-files')
    , del = require('del')
    , merge = require('merge-stream')
    ;

var bowerPkg = require("./bower.json");

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
});

/* Serve the dist/ directory. */
gulp.task("connect:dist", function () {
    return connect.server({
        root: ["dist"],
        port: 9090,
        livereload: true
    })
});

/* Server dist directory and rebuild/reload on changes in app/. */
gulp.task("watch:dist", ["connect:dist"], function () {
    gulp.watch(["app/**/*"], ["dist", function () {
        gulp.src("app/**/*").pipe(connect.reload());
    }]);
});

/* Remove the dist directory. */
gulp.task("clean-dist", function (cb) {
    del(["dist"], cb);
});

var copyApp = function () {
    return gulp.src(["app/**/*", "!app/bower_components/**/*"])
        .pipe(gulp.dest("./dist/"));
};

var copyBowerComponents = function () {
    return gulp.src(bowerFiles(), {base: "./app/"})
        .pipe(gulp.dest("./dist/"));
};

gulp.task("dist", ["clean-dist"], function () {
    return merge(copyApp(), copyBowerComponents());
});
