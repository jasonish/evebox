var webpack = require("webpack");
var webpackMerge = require('webpack-merge');
var commonConfig = require("./webpack.common.js");

const ENV = "production";

module.exports = webpackMerge(commonConfig, {

    plugins: [
        new webpack.DefinePlugin({
            "process.env": {
                "ENV": JSON.stringify(ENV)
            }
        })
    ],

    output: {
        path: "../resources/public",
        filename: "[name].js"
    }

});

