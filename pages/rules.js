// Progressive enhancement for rules.html: without this script every rule is
// listed and every filter control is simply inert; with it, the search box
// and severity/tag/auto-fix checkboxes filter the table client-side against
// the data-search/data-severity/data-tags/data-fixable attributes build.py
// wrote onto each row.
(function () {
  "use strict";

  function init() {
    var table = document.getElementById("rules-table");
    if (!table) {
      return;
    }

    var rows = Array.prototype.slice.call(table.querySelectorAll("tbody tr"));
    var searchInput = document.getElementById("rules-search");
    var severityInputs = Array.prototype.slice.call(document.querySelectorAll(".rules-severity-filter"));
    var tagInputs = Array.prototype.slice.call(document.querySelectorAll(".rules-tag-filter"));
    var fixableInput = document.getElementById("rules-fixable-filter");
    var countEl = document.getElementById("rules-count");

    function checkedValues(inputs) {
      return inputs
        .filter(function (input) {
          return input.checked;
        })
        .map(function (input) {
          return input.value;
        });
    }

    function applyFilters() {
      var query = (searchInput ? searchInput.value : "").trim().toLowerCase();
      var severities = checkedValues(severityInputs);
      var tags = checkedValues(tagInputs);
      var fixableOnly = fixableInput ? fixableInput.checked : false;
      var visible = 0;

      rows.forEach(function (row) {
        var rowTags = row.dataset.tags ? row.dataset.tags.split(" ") : [];
        var matches =
          (!query || row.dataset.search.indexOf(query) !== -1) &&
          severities.indexOf(row.dataset.severity) !== -1 &&
          tags.some(function (tag) {
            return rowTags.indexOf(tag) !== -1;
          }) &&
          (!fixableOnly || row.dataset.fixable === "true");

        row.hidden = !matches;
        if (matches) {
          visible += 1;
        }
      });

      if (countEl) {
        countEl.textContent = "Showing " + visible + " of " + rows.length + " rules";
      }
    }

    if (searchInput) {
      searchInput.addEventListener("input", applyFilters);
    }
    severityInputs.concat(tagInputs).forEach(function (input) {
      input.addEventListener("change", applyFilters);
    });
    if (fixableInput) {
      fixableInput.addEventListener("change", applyFilters);
    }

    applyFilters();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
