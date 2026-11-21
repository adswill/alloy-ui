(function(){
  var root=document.documentElement;
  var b=document.getElementById("theme");
  if(b)b.addEventListener("click",function(){
    var dark=root.dataset.theme?root.dataset.theme==="dark":matchMedia("(prefers-color-scheme: dark)").matches;
    var next=dark?"light":"dark";root.dataset.theme=next;
    try{localStorage.setItem("alloy-theme",next)}catch(e){}
  });
  document.querySelectorAll(".copy").forEach(function(c){
    c.addEventListener("click",function(){
      var t=c.getAttribute("data-copy");
      (navigator.clipboard?navigator.clipboard.writeText(t):Promise.reject()).then(function(){c.textContent="Copied";setTimeout(function(){c.textContent="Copy"},1400)}).catch(function(){});
    });
  });
  var tabs=document.querySelectorAll('.tabs [role=tab]');
  tabs.forEach(function(t){t.addEventListener("click",function(){
    tabs.forEach(function(x){x.setAttribute("aria-selected",x===t)});
    document.querySelectorAll(".panel").forEach(function(p){p.hidden=p.id!==t.getAttribute("aria-controls")});
  })});
})();
