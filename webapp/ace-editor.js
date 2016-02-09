/* Copyright (c) 2014-2015 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

import angular from "angular";
import "brace";
import "brace/mode/json";
import "brace/mode/lua";

(function() {

    angular.module("app").directive("aceEditor", aceEditor);

    function aceEditor() {

        let template = '<div id="ace-editor"></div>';

        controller.$inject = ["$scope"];

        function controller($scope) {

            var vm = this;

            var resize = function(editor) {
                var height = editor.getSession().getScreenLength()
                    * editor.renderer.lineHeight
                    + editor.renderer.scrollBar.getWidth()
                    + 30; // For some extra bottom buffer.
                $("#ace-editor").height(height.toString() + "px");
                editor.resize();
            };

            var activate = function() {

                var editor = ace.edit("ace-editor");

                // Suppresses a deprecation warning.
                editor.$blockScrolling = Infinity;

                editor.setValue(vm.content, -1);

                resize(editor);

                if (vm.readOnly != undefined) {
                    editor.setReadOnly(vm.readOnly);
                }

                // If mode is set we'll use.
                if (vm.mode != undefined) {
                    editor.getSession().setMode("ace/mode/" + vm.mode);
                }
                // But if it isn't, and a filename is set we'll try that.
                else if (vm.filename != undefined) {
                    console.log(
                        "ace-editor: determing mode from filename " + vm.filename);
                    var mode = undefined;
                    if (_.endsWith(vm.filename, ".lua")) {
                        mode = "lua";
                    }
                    if (mode !== undefined) {
                        editor.getSession().setMode("ace/mode/" + mode);
                    }
                }

                if (vm.showPrintMargin != undefined) {
                    editor.setShowPrintMargin(vm.showPrintMargin);
                }
                if (vm.highlightActiveLine != undefined) {
                    editor.setHighlightActiveLine(vm.highlightActiveLine);
                }
                if (vm.useWrapMode != undefined) {
                    editor.getSession().setUseWrapMode(vm.useWrapMode);
                }

                $scope.$watch('vm.content', function() {
                    editor.setValue(vm.content, -1);
                    resize(editor);
                });

                resize(editor);
            };

            if (vm.content != undefined) {
                activate();
            }
            else {
                $scope.$watch('vm.content', function() {
                    if (vm.content != undefined) {
                        activate();
                    }
                });
            }

        }

        return {
            restrict: "AE",
            scope: {
                content: "=",
                config: "=",
                readOnly: "=",
                mode: "@",
                showPrintMargin: "=",
                highlightActiveLine: "=",
                useWrapMode: "=",
                filename: "="
            },
            template: template,
            controller: controller,
            controllerAs: "vm",
            bindToController: true
        }
    }

})();

