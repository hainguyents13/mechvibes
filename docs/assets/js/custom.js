// Custom Scripts for Array Template //

jQuery(function($) {
  'use strict';

  // get the value of the bottom of the #main element by adding the offset of that element plus its height, set it as a variable
  var mainbottom = $('#main').offset().top;

  // on scroll,
  $(window).on('scroll', function() {
    // we round here to reduce a little workload
    stop = Math.round($(window).scrollTop());
    if (stop > mainbottom) {
      $('.navbar').addClass('past-main');
      $('.navbar').addClass('effect-main');
    } else {
      $('.navbar').removeClass('past-main');
    }
  });

  // Collapse navbar on click
  $(document).on('click.nav', '.navbar-collapse.in', function(e) {
    if ($(e.target).is('a')) {
      $(this)
        .removeClass('in')
        .addClass('collapse');
    }
  });

  /*-----------------------------------
    ----------- Scroll To Top -----------
    ------------------------------------*/

  $(window).on('scroll', function() {
    if ($(this).scrollTop() > 1000) {
      $('#back-top').fadeIn();
    } else {
      $('#back-top').fadeOut();
    }
  });
  // scroll body to 0px on click
  $('#back-top').on('click', function() {
    $('#back-top').tooltip('hide');
    $('body,html').animate(
      {
        scrollTop: 0,
      },
      1500,
    );
    return false;
  });

  /* ------ jQuery for Easing min -- */
  (function($) {
    'use strict'; // Start of use strict

    // Smooth scrolling using jQuery easing
    $('a.js-scroll-trigger[href*="#"]:not([href="#"])').on('click', function() {
      if (location.pathname.replace(/^\//, '') == this.pathname.replace(/^\//, '') && location.hostname == this.hostname) {
        var target = $(this.hash);
        target = target.length ? target : $('[name=' + this.hash.slice(1) + ']');
        if (target.length) {
          $('html, body').animate(
            {
              scrollTop: target.offset().top - 54,
            },
            1000,
            'easeInOutExpo',
          );
          return false;
        }
      }
    });

    // Closes responsive menu when a scroll trigger link is clicked
    $('.js-scroll-trigger').on('click', function() {
      $('.navbar-collapse').collapse('hide');
    });

    // Activate scrollspy to add active class to navbar items on scroll
    $('body').scrollspy({
      target: '#mainNav',
      offset: 54,
    });
  })(jQuery); // End of use strict

  /* ----- Counter Up ----- */

  /*----- Subscription Form ----- */

  $(document).ready(function() {
    $.ajax('https://api.github.com/repos/hainguyents13/mechvibes/releases').done(rs => {
      let total_downloads = 0;
      for (let rl of rs) {
        for (let asset of rl.assets) {
          total_downloads += asset.download_count;
        }
      }

      const latest = rs.shift();
      let tag_name = latest.tag_name;
      $(`.version`).html(tag_name);

      const dl = {
        exe: latest.assets.find(a => a.content_type == 'application/x-msdownload'),
        zip: latest.assets.find(a => a.content_type == 'application/zip' || a.content_type == 'application/x-zip-compressed'),
        dmg: latest.assets.find(a => a.content_type == 'application/octet-stream'),
        ai: latest.assets.find(a => a.content_type == 'application/x-iso9660-appimage'),
      };

      for (let i in dl) {
        if (dl[i]) {
          const size = formatBytes(dl[i].size);
          $(`.dl-${i}`).attr('href', dl[i].browser_download_url);
          $(`.dl-${i}-size`).html(size);
        }
      }

      $(`.counter`).html(total_downloads.toLocaleString());
      $('.counter').counterUp({ delay: 10, time: 1000 });
    });
  });
});

function formatBytes(a, b) {
  if (0 == a) return '0 Bytes';
  var c = 1024,
    d = b || 2,
    e = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'],
    f = Math.floor(Math.log(a) / Math.log(c));
  return parseFloat((a / Math.pow(c, f)).toFixed(d)) + ' ' + e[f];
}
