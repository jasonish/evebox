'use strict';

angular.module("app")
    .controller("ConfigController",
    ["$modalInstance", "Config", ConfigController]);

function ConfigController($modalInstance, Config) {
    var mv = this;
    mv.$modalInstance = $modalInstance;
    mv.config = Config;
}

ConfigController.prototype.ok = function() {
    this.config.save();
    this.$modalInstance.close();
};

ConfigController.prototype.cancel = function() {
    this.$modalInstance.dismiss();
};
