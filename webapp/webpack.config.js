var webpack = require("webpack");

const ENV = process.env.RUN_MODE || "production";

module.exports = {

    devtool: "source-map",

    entry: {
        app: './src/main.ts'
    },

    resolve: {
        extensions: [
            '',
            '.js',
            '.ts'
        ]
    },

    module: {
        loaders: [
            {
                test: /\.ts$/,
                loaders: ['awesome-typescript-loader', 'angular2-template-loader']
            },
            {
                test: /\.css$/,
                loader: "style!css"
            },
            {
                test: /\.scss$/,
                loader: "style!css!sass"
            },
            {
                test: /\.html$/,
                loader: "html"
            },
            {
                test: /(\.eot(\?.*)?$)|(\.woff(\?.*)?$)|(\.woff2(\?.*)?$)|(\.ttf(\?.*)?$)|(\.svg(\?.*)?$)/,
                loader: "url"
            }
        ]
    },

    plugins: [
        new webpack.DefinePlugin({
            "process.env": {
                "ENV": JSON.stringify(ENV)
            }
        })
    ],

    output: {
        path: "../public",
        filename: "bundle.js"
    }
};