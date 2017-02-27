var webpack = require("webpack");
var HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {

    entry: {
        app: './src/main.ts'
    },

    resolve: {
        extensions: [
            '.js',
            '.ts'
        ]
    },

    module: {
        loaders: [
            {
                test: /\.ts$/,
                loaders: ['ts-loader', 'angular2-template-loader']
            },
            {
                test: /\.css$/,
                loader: "style-loader!css-loader"
            },
            {
                test: /\.scss$/,
                loader: "style-loader!css-loader!sass-loader"
            },
            {
                test: /\.html$/,
                loader: "html-loader"
            },
            {
                test: /(\.eot(\?.*)?$)|(\.woff(\?.*)?$)|(\.woff2(\?.*)?$)|(\.ttf(\?.*)?$)|(\.svg(\?.*)?$)/,
                loader: "url-loader"
            }
        ]
    },

    devtool: "source-map",

    plugins: [
        // Workaround for angular/angular#11580
        new webpack.ContextReplacementPlugin(
            // The (\\|\/) piece accounts for path separators in *nix and Windows
            /angular(\\|\/)core(\\|\/)(esm(\\|\/)src|src)(\\|\/)linker/,
            "./src",
            //helpers.root('./src'), // location of your src
            {} // a map of your routes
        ),

        new HtmlWebpackPlugin({
            template: 'src/index.html'
        })
    ]

}