// Valocraft website — nav toggle + gallery lightbox
(function () {
  "use strict";

  // Mobile navigation
  var toggle = document.querySelector(".nav-toggle");
  var links = document.querySelector(".nav-links");
  if (toggle && links) {
    toggle.addEventListener("click", function () {
      var open = links.classList.toggle("open");
      toggle.setAttribute("aria-expanded", open ? "true" : "false");
    });
  }

  // Gallery lightbox
  var lightbox = document.getElementById("lightbox");
  if (lightbox) {
    var img = lightbox.querySelector("img");
    var close = lightbox.querySelector(".lightbox-close");

    document.querySelectorAll(".gallery-item").forEach(function (item) {
      item.addEventListener("click", function () {
        var full = item.getAttribute("data-full");
        if (!full) return;
        img.src = full;
        img.alt = item.querySelector("img").alt || "Enlarged screenshot";
        lightbox.classList.add("open");
        document.body.style.overflow = "hidden";
      });
    });

    function dismiss() {
      lightbox.classList.remove("open");
      document.body.style.overflow = "";
    }

    close.addEventListener("click", dismiss);
    lightbox.addEventListener("click", function (e) {
      if (e.target === lightbox) dismiss();
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Escape") dismiss();
    });
  }
})();
