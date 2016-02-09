var webpack = require("webpack");
var path = require("path");

module.exports = {
    entry: [
        "./webapp/main.js"
    ]
    ,
    output: {
        path: "public",
        filename: "bundle.js"
    }
    ,
    module: {
        loaders: [
            {
                test: /\.js$/,
                include: [
                    path.resolve(__dirname, "webapp")
                ],
                loaders: [
                    "babel?plugins=babel-plugin-object-assign"
                ]
            }
            ,
            {
                test: /\.ts$/,
                loader: "ts"
            }
            ,
            {
                test: /\.css$/,
                loader: "style!css"
            }
            ,
            {
                test: /\.scss$/,
                loader: "style!css!sass"
            }
            ,
            {
                test: /\.html$/,
                loader: "html"
            }
            ,
            {
                test: /(\.eot(\?.*)?$)|(\.woff(\?.*)?$)|(\.woff2(\?.*)?$)|(\.ttf(\?.*)?$)|(\.svg(\?.*)?$)/,
                loader: "url"
            }
        ]
    }
    ,
    plugins: [
        new webpack.ProvidePlugin({
            $: "jquery",
            jQuery: "jquery"
        })
    ]
};
