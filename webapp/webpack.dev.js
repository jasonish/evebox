var webpack = require("webpack");
var webpackMerge = require('webpack-merge');
var commonConfig = require("./webpack.common.js");

var path = require('path');
var _root = path.resolve(__dirname, '..');
function root(args) {
    args = Array.prototype.slice.call(arguments, 0);
    return path.join.apply(path, [_root].concat(args));
}

module.exports = webpackMerge(commonConfig, {

    output: {
        path: root("../resources/public"),
        filename: "[name].js",
        publicPath: "http://localhost:58080"
    }

});

