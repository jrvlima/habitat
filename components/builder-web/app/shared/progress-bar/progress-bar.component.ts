// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { Component } from "@angular/core";
import {AppStore} from "../../AppStore";

@Component({
    selector: "hab-progress-bar",
    template: `
    <div class="progress-bar-wrapper" *ngIf="loading">
      <md-progress-bar mode="indeterminate"></md-progress-bar>
    </div>`
})

export class ProgressBarComponent {
  constructor(private store: AppStore) { }
  get loading() {
    const state = this.store.getState();
    return (
      state.origins.ui.mine.loading &&
      state.packages.ui.visible.loading &&
      state.projects.current.ui.loading
    ); }
}
