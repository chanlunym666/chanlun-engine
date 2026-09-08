//+------------------------------------------------------------------+
//|                                    chanlun.mq4                     |
//|   buf0-1: 笔     (暗绿)                                           |
//|   buf2-3: 线段   (中兰)                                           |
//|   buf4-5: 大段   (深天蓝)                                         |
//|   buf6-7: 笔轨道 (暗灰)                                           |
//|   buf8-9: 线段轨道(品红)                                          |
//|   buf11-12:大段轨道(Aqua)                                          |
//|   buf14-15:高级段(亮浅灰)                                          |
//+------------------------------------------------------------------+
#property strict
#property indicator_chart_window
#property indicator_buffers 16
#property indicator_color1 clrDarkGreen
#property indicator_color2 clrDarkGreen
#property indicator_color3 clrMediumOrchid
#property indicator_color4 clrMediumOrchid
#property indicator_color5 clrDeepSkyBlue
#property indicator_color6 clrDeepSkyBlue
#property indicator_color7 clrDimGray
#property indicator_color8 clrDimGray
#property indicator_color9 clrMagenta
#property indicator_color10 clrMagenta
#property indicator_color11 clrMagenta
#property indicator_color12 clrAqua
#property indicator_color13 clrAqua
#property indicator_color14 clrAqua
#property indicator_color15 clrDarkGray
#property indicator_color16 clrDarkGray
#property indicator_width1 2
#property indicator_width2 2
#property indicator_width3 2
#property indicator_width4 2
#property indicator_width5 2
#property indicator_width6 2
#property indicator_width7 1
#property indicator_width8 1
#property indicator_width9 1
#property indicator_width10 1
#property indicator_width11 1
#property indicator_width12 2
#property indicator_width13 2
#property indicator_width14 2
#property indicator_width15 2
#property indicator_width16 2

input int ChanlunStrokeCases = 0; // 笔: 0全开 / 3关c3 / 4关c4 / 5关c34
input int ChanlunLevelCases  = 0; // 线段~高级段: 0全开 / 3关c3 / 4关c4 / 5关c34
input int ChanlunBandDisplay = 0; // 1不显笔轨 / 2不显线轨 / 3不显大轨 / 4不显三轨
#import "slzs_chanlun_mt4.dll"
   int chanlun_init(int rates_total, double &highs[], double &lows[]);
   int chanlun_set_cases(int stroke_param, int levels_param);
   int chanlun_get_strokes(double &upBuf[], double &downBuf[]);
   int chanlun_get_segments(double &upBuf[], double &downBuf[]);
   int chanlun_get_bigsegments(double &upBuf[], double &downBuf[]);
   int chanlun_get_stroke_bands(double &upBuf[], double &downBuf[]);
   int chanlun_get_segment_bands(double &upBuf[], double &downBuf[], double &midBuf[]);
   int chanlun_get_bigseg_bands(double &upBuf[], double &downBuf[], double &midBuf[]);
   int chanlun_get_superior_segments(double &upBuf[], double &downBuf[]);
   int chanlun_markers_compute();
   double chanlun_markers_get(int index, int &bar, int &kind);
   int chanlun_zhongshus_compute();
   int chanlun_zhongshus_get(int index, int &start_bar, int &end_bar, double &zg, double &zd);
#import

double bsUp[], bsDown[];      // 笔
double segUp[], segDown[];    // 线段
double bigUp[], bigDown[];    // 大段
double bandUp[], bandDown[];  // 笔轨道
double sgbUp[], sgbDown[];    // 线段轨道
double sgbMid[];              // 线段中间线(不画线)
double bgbUp[], bgbDown[];    // 大段轨道
double bgbMid[];              // 大段中间线(不画线)
double supUp[], supDown[];    // 高级段

int OnInit()
{
   SetIndexBuffer(0, bsUp);      SetIndexStyle(0, DRAW_LINE);  SetIndexLabel(0, "笔↑");
   SetIndexBuffer(1, bsDown);    SetIndexStyle(1, DRAW_LINE);  SetIndexLabel(1, "笔↓");
   SetIndexBuffer(2, segUp);     SetIndexStyle(2, DRAW_LINE);  SetIndexLabel(2, "线段↑");
   SetIndexBuffer(3, segDown);   SetIndexStyle(3, DRAW_LINE);  SetIndexLabel(3, "线段↓");
   SetIndexBuffer(4, bigUp);     SetIndexStyle(4, DRAW_LINE);  SetIndexLabel(4, "大段↑");
   SetIndexBuffer(5, bigDown);   SetIndexStyle(5, DRAW_LINE);  SetIndexLabel(5, "大段↓");
   SetIndexBuffer(6, bandUp);    SetIndexStyle(6, DRAW_LINE);  SetIndexLabel(6, "笔上轨");
   SetIndexBuffer(7, bandDown);  SetIndexStyle(7, DRAW_LINE);  SetIndexLabel(7, "笔下轨");
   SetIndexBuffer(8, sgbUp);     SetIndexStyle(8, DRAW_LINE);  SetIndexLabel(8, "线段上轨");
   SetIndexBuffer(9, sgbDown);   SetIndexStyle(9, DRAW_LINE);  SetIndexLabel(9, "线段下轨");
   SetIndexBuffer(10, sgbMid);   SetIndexStyle(10, DRAW_NONE); SetIndexLabel(10, "线段中轨");
   SetIndexBuffer(11, bgbUp);    SetIndexStyle(11, DRAW_LINE); SetIndexLabel(11, "大段上轨");
   SetIndexBuffer(12, bgbDown);  SetIndexStyle(12, DRAW_LINE); SetIndexLabel(12, "大段下轨");
   SetIndexBuffer(13, bgbMid);   SetIndexStyle(13, DRAW_NONE); SetIndexLabel(13, "大段中轨");
   SetIndexBuffer(14, supUp);    SetIndexStyle(14, DRAW_LINE); SetIndexLabel(14, "高级段↑");
   SetIndexBuffer(15, supDown);  SetIndexStyle(15, DRAW_LINE); SetIndexLabel(15, "高级段↓");
   return(INIT_SUCCEEDED);
}

int OnCalculate(const int rates_total,
                const int prev_calculated,
                const datetime &time[],
                const double &open[],
                const double &high[],
                const double &low[],
                const double &close[],
                const long &tick_volume[],
                const long &volume[],
                const int &spread[])
{
   if(prev_calculated == rates_total)
   {
      // 【守卫】MT4 重启/缓存恢复时 buffer 可能全空，强制重算
      if(bsUp[rates_total-1] == EMPTY_VALUE && bsDown[rates_total-1] == EMPTY_VALUE
         && segUp[rates_total-1] == EMPTY_VALUE && segDown[rates_total-1] == EMPTY_VALUE
         && bigUp[rates_total-1] == EMPTY_VALUE && bigDown[rates_total-1] == EMPTY_VALUE
         && supUp[rates_total-1] == EMPTY_VALUE && supDown[rates_total-1] == EMPTY_VALUE)
      {
         // buffer 未初始化 → 走下方全量计算路径
      }
      else
      {
         return(rates_total);
      }
   }

   double h[], l[];
   ArrayResize(h, rates_total);
   ArrayResize(l, rates_total);
   for(int i = 0; i < rates_total; i++)
   {  h[i] = high[rates_total-1-i]; l[i] = low[rates_total-1-i]; }

   chanlun_set_cases(ChanlunStrokeCases, ChanlunLevelCases);
   int initOk = chanlun_init(rates_total, h, l);
   if(initOk == 0) return(rates_total);

   ArrayInitialize(bsUp, EMPTY_VALUE);   ArrayInitialize(bsDown, EMPTY_VALUE);
   ArrayInitialize(segUp, EMPTY_VALUE);  ArrayInitialize(segDown, EMPTY_VALUE);
   ArrayInitialize(bigUp, EMPTY_VALUE);  ArrayInitialize(bigDown, EMPTY_VALUE);
   ArrayInitialize(bandUp, EMPTY_VALUE); ArrayInitialize(bandDown, EMPTY_VALUE);
   ArrayInitialize(sgbUp, EMPTY_VALUE);  ArrayInitialize(sgbDown, EMPTY_VALUE);
   ArrayInitialize(sgbMid, EMPTY_VALUE);
   ArrayInitialize(bgbUp, EMPTY_VALUE);  ArrayInitialize(bgbDown, EMPTY_VALUE);
   ArrayInitialize(bgbMid, EMPTY_VALUE);
   ArrayInitialize(supUp, EMPTY_VALUE);  ArrayInitialize(supDown, EMPTY_VALUE);

   int bs = chanlun_get_strokes(bsUp, bsDown);
   int xd = chanlun_get_segments(segUp, segDown);
   int dd = chanlun_get_bigsegments(bigUp, bigDown);
   int gd = chanlun_get_stroke_bands(bandUp, bandDown);
   int sg = chanlun_get_segment_bands(sgbUp, sgbDown, sgbMid);
   int bg = chanlun_get_bigseg_bands(bgbUp, bgbDown, bgbMid);
   int sup = chanlun_get_superior_segments(supUp, supDown);
   // 轨道显示开关 (0全显/1不显笔轨/2不显线轨/3不显大轨/4不显三轨)
   if(ChanlunBandDisplay == 1 || ChanlunBandDisplay == 4)
   { ArrayInitialize(bandUp, EMPTY_VALUE); ArrayInitialize(bandDown, EMPTY_VALUE); }
   if(ChanlunBandDisplay == 2 || ChanlunBandDisplay == 4)
   { ArrayInitialize(sgbUp, EMPTY_VALUE); ArrayInitialize(sgbDown, EMPTY_VALUE); }
   if(ChanlunBandDisplay == 3 || ChanlunBandDisplay == 4)
   { ArrayInitialize(bgbUp, EMPTY_VALUE); ArrayInitialize(bgbDown, EMPTY_VALUE); }

   // ═══ 大段中枢矩形 (深天蓝 + 背景半透明填充) ═══
   for(int oi3 = ObjectsTotal()-1; oi3 >= 0; oi3--) { string on3 = ObjectName(oi3);
      if(StringFind(on3, "ZS_") == 0) ObjectDelete(on3); }
   int zc = chanlun_zhongshus_compute();
   for(int zi = 0; zi < zc; zi++) {
      int zsb; int zeb; double zzg; double zzd;
      int zok = chanlun_zhongshus_get(zi, zsb, zeb, zzg, zzd);
      if(zok == 0 || zsb < 0 || zeb < zsb || zeb >= rates_total) continue;
      datetime zt1 = time[rates_total-1-zsb];
      datetime zt2 = time[rates_total-1-zeb];
      if(zt2 <= zt1) continue;
      string zid = "ZS_" + IntegerToString(zsb) + "_" + IntegerToString(zeb);
      ObjectCreate(0, zid, OBJ_RECTANGLE, 0, zt1, zzg, zt2, zzd);
      ObjectSetInteger(0, zid, OBJPROP_COLOR, clrDeepSkyBlue);
      ObjectSetInteger(0, zid, OBJPROP_WIDTH, 2);
      ObjectSetInteger(0, zid, OBJPROP_STYLE, STYLE_SOLID);
      ObjectSetInteger(0, zid, OBJPROP_BACK, true);
   }

   // ═══ 二买/二卖/三买/三卖 文字标记 (买=红下方, 卖=绿上方) ═══
   for(int oi2 = ObjectsTotal()-1; oi2 >= 0; oi2--) { string on2 = ObjectName(oi2);
      if(StringFind(on2, "BM_") == 0) ObjectDelete(on2); }
   int mc = chanlun_markers_compute();
   string bmNames[4] = {"二买", "二卖", "三买", "三卖"};
   for(int mi = 0; mi < mc; mi++) {
      int bar; int kind; double price = chanlun_markers_get(mi, bar, kind);
      if(kind < 0 || kind > 3 || bar < 0 || bar >= rates_total) continue;
      string bmName = "BM_" + IntegerToString(bar) + "_" + IntegerToString(kind);
      ObjectCreate(0, bmName, OBJ_TEXT, 0, time[rates_total-1-bar], price);
      ObjectSetString(0, bmName, OBJPROP_TEXT, bmNames[kind]);
      bool isBuy = (kind == 0 || kind == 2);
      ObjectSetInteger(0, bmName, OBJPROP_COLOR, isBuy ? clrRed : clrLime);
      ObjectSetInteger(0, bmName, OBJPROP_FONTSIZE, 10);
      ObjectSetInteger(0, bmName, OBJPROP_ANCHOR, isBuy ? ANCHOR_UPPER : ANCHOR_LOWER);
   }
   ChartRedraw();

   return(rates_total);
}
//+------------------------------------------------------------------+
