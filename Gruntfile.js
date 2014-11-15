// Generated on 2014-04-25 using generator-angular 0.8.0
'use strict';

// # Globbing
// for performance reasons we're only matching one level down:
// 'test/spec/{,*/}*.js'
// use this if you want to recursively match all subfolders:
// 'test/spec/**/*.js'

var VERSION = "0.4.0dev"

module.exports = function (grunt) {

    var pkg = grunt.file.readJSON('package.json');
    var dist = "evebox-" + VERSION;

    // Load grunt tasks automatically
    require('load-grunt-tasks')(grunt);

    // Time how long tasks take. Can help when optimizing build times
    require('time-grunt')(grunt);

    // Define the configuration for all the tasks
    grunt.initConfig({

        // Watches files for changes and runs tasks based on the changed files
        watch: {
            bower: {
                files: ['bower.json'],
                tasks: ['bowerInstall']
            },
            js: {
                files: ['app/scripts/{,*/}*.js'],
                tasks: [],
                options: {
                    livereload: true
                }
            },
            styles: {
                files: ['app/styles/{,*/}*.css'],
                tasks: ['newer:copy:styles', 'autoprefixer']
            },
            gruntfile: {
                files: ['Gruntfile.js']
            },
            livereload: {
                options: {
                    livereload: '<%= connect.options.livereload %>'
                },
                files: [
                    'app/{,*/}*.html',
                    '.tmp/styles/{,*/}*.css',
                    'app/images/{,*/}*.{png,jpg,jpeg,gif,webp,svg}'
                ]
            }
        },

        // The actual grunt server settings
        connect: {
            options: {
                port: 9000,
                // Change this to '0.0.0.0' to access the server from outside.
                hostname: 'localhost',
                livereload: 35729
            },
            livereload: {
                options: {
                    open: true,
                    base: [
                        '.tmp',
                        'app'
                    ]
                }
            },
            dist: {
                options: {
                    base: 'dist'
                }
            },
            test: {
                options: {
                    port: 9001,
                    base: [
                        '.tmp',
                        'test',
                        'app'
                    ]
                }
            }
        },

        // Empties folders to start fresh
        clean: {
            dist: {
                files: [
                    {
                        dot: true,
                        src: [
                            '.tmp',
                            'dist/*',
                            '!dist/.git*'
                        ]
                    }
                ]
            },
            server: '.tmp'
        },

        // Add vendor prefixed styles
        autoprefixer: {
            options: {
                browsers: ['last 1 version']
            },
            dist: {
                files: [
                    {
                        expand: true,
                        cwd: '.tmp/styles/',
                        src: '{,*/}*.css',
                        dest: '.tmp/styles/'
                    }
                ]
            }
        },

        // Automatically inject Bower components into the app
        bowerInstall: {
            app: {
                src: ['app/index.html'],
                ignorePath: 'app/'
            }
        },

        // Renames files for browser caching purposes
        rev: {
            dist: {
                files: {
                    src: [
                        'dist/scripts/{,*/}*.js',
                        'dist/styles/{,*/}*.css',
                        'dist/images/{,*/}*.{png,jpg,jpeg,gif,webp,svg}',
                        'dist/styles/fonts/*'
                    ]
                }
            }
        },

        // Reads HTML for usemin blocks to enable smart builds that automatically
        // concat, minify and revision files. Creates configurations in memory so
        // additional tasks can operate on them
        useminPrepare: {
            html: 'app/index.html',
            options: {
                dest: 'dist',
                flow: {
                    html: {
                        steps: {
                            js: ['concat', 'uglifyjs'],
                            css: ['cssmin']
                        },
                        post: {}
                    }
                }
            }
        },

        // Performs rewrites based on rev and the useminPrepare configuration
        usemin: {
            html: ['dist/{,*/}*.html'],
            css: ['dist/styles/{,*/}*.css'],
            options: {
                assetsDirs: ['dist']
            }
        },

        // The following *-min tasks produce minified files in the dist folder
        cssmin: {
        },

        imagemin: {
            dist: {
                files: [
                    {
                        expand: true,
                        cwd: 'app/images',
                        src: '{,*/}*.{png,jpg,jpeg,gif}',
                        dest: 'dist/images'
                    }
                ]
            }
        },

        svgmin: {
            dist: {
                files: [
                    {
                        expand: true,
                        cwd: 'app/images',
                        src: '{,*/}*.svg',
                        dest: 'dist/images'
                    }
                ]
            }
        },

        htmlmin: {
            dist: {
                options: {
                    collapseWhitespace: true,
                    collapseBooleanAttributes: true,
                    removeCommentsFromCDATA: true,
                    removeOptionalTags: true
                },
                files: [
                    {
                        expand: true,
                        cwd: 'dist',
                        src: ['*.html',
                            'views/{,*/}*.html',
                            'templates/{,*/}*.html'],
                        dest: 'dist'
                    }
                ]
            }
        },

        // ngmin tries to make the code safe for minification automatically by
        // using the Angular long form for dependency injection. It doesn't work on
        // things like resolve or inject so those have to be done manually.
        ngmin: {
            dist: {
                files: [
                    {
                        expand: true,
                        cwd: '.tmp/concat/scripts',
                        src: '*.js',
                        dest: '.tmp/concat/scripts'
                    }
                ]
            }
        },

        // Copies remaining files to places other tasks can use
        copy: {
            dist: {
                files: [
                    {
                        expand: true,
                        dot: true,
                        cwd: 'app',
                        dest: 'dist',
                        src: [
                            '*.{ico,png,txt}',
                            '.htaccess',
                            '*.html',
                            'views/{,*/}*.html',
                            'templates/{,*/}*.html',
                            'images/{,*/}*.{webp}',
                            'fonts/*',
                            'sample-config.js'
                        ]
                    },
                    {
                        expand: true,
                        cwd: '.tmp/images',
                        dest: 'dist/images',
                        src: ['generated/*']
                    },

                    // add this rule to copy the fonts:
                    {
                        expand: true,
                        flatten: true,
                        cwd: 'app',
                        dest: 'dist/fonts',
                        src: ['bower_components/bootstrap/dist/fonts/*.*']
                    },

                    {
                        expand: true,
                        cwd: '.',
                        dest: 'dist',
                        src: ["README.md", "LICENSE.txt"]
                    }

                ]
            },
            styles: {
                expand: true,
                cwd: 'app/styles',
                dest: '.tmp/styles/',
                src: '{,*/}*.css'
            }
        },

        // Run some tasks in parallel to speed up the build process
        concurrent: {
            server: [
                'copy:styles'
            ],
            dist: [
                'copy:styles',
                'imagemin',
                'svgmin'
            ],
            test: [
                'copy:styles'
            ]
        },

        uglify: {
            options : {
                beautify : true,
                mangle   : false
            }
        },

        compress: {
            build: {
                options: {archive: 'build/' + dist + '.zip', mode: 'zip'},
                src: ['**'], cwd: 'dist', expand: true, dot: true, dest: dist + '/'
            }
        },

        // Test settings
        karma: {
            unit: {
                configFile: 'karma.conf.js',
                singleRun: true
            }
        },

        replace: {
            another_example: {
                src: ['app/templates/about.html'],
                overwrite: true,
                replacements: [
                    {
                        from: /\<span id=\"version\"\>.*\<\/span\>/,
                        to: "<span id=\"version\">" + VERSION + "</span>"
                    }
                ]
            }
        }

    });

    grunt.registerTask('serve', function (target) {
        if (target === 'dist') {
            return grunt.task.run(['build', 'connect:dist:keepalive']);
        }

        grunt.task.run([
            'clean:server',
            'bowerInstall',
            'concurrent:server',
            'autoprefixer',
            'connect:livereload',
            'watch'
        ]);
    });

    grunt.registerTask('server', function (target) {
        grunt.log.warn('The `server` task has been deprecated. Use `grunt serve` to start a server.');
        grunt.task.run(['serve:' + target]);
    });

    grunt.registerTask('test', [
        'clean:server',
        'concurrent:test',
        'autoprefixer',
        'connect:test',
        'karma'
    ]);

    grunt.registerTask('build', [
        'clean:dist',
        'bowerInstall',
        'replace',
        'useminPrepare',
        'concurrent:dist',
        'autoprefixer',
        'concat',
        'ngmin',
        'copy:dist',
        'cssmin',
        'uglify',
        'rev',
        'usemin',
        'htmlmin'
    ]);

    grunt.registerTask('default', [
        'build'
    ]);

    grunt.registerTask("package", [
        "build",
        "compress"
    ]);
};
