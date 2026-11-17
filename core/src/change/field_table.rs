// SPDX-License-Identifier: GPL-2.0-or-later

// Stable diagnostic index, patch variant, Model field/type, storage policy and classes.
// Group boundaries preserve the public Patch variants; row order preserves field emission.
macro_rules! field_table {
    ($generate:ident) => {
        $generate! {
            Channels0 channels0 {
                0 Ch(ch: [Channel; NCH]) clone [NONE];
                1 Active(active: usize) copy [NONE];
                59 ChLabel(ch_label: [String; NCH]) strings [NONE];
                67 FineV(fine_v: bool) copy [NONE];
                70 VExpandGnd(v_expand_gnd: bool) copy [NONE];
            }
            Timebase0 timebase0 {
                2 Tscale(tscale: usize) copy [ACQ|STATS];
                3 Delay(delay: f32) copy [ACQ|STATS];
                68 FineT(fine_t: bool) copy [NONE];
                69 Tfine(tfine: f32) copy [ACQ|STATS];
                71 HExpand(h_expand: HExpand) copy [NONE];
                117 Zoom(zoom: bool) copy [NONE];
                119 Zscale(zscale: usize) copy [NONE];
                120 Zdelay(zdelay: f32) copy [NONE];
                118 AutoRoll(auto_roll: bool) copy [ACQ|STATS];
            }
            Trigger0 trigger0 {
                4 TrigSrc(trig_src: usize) copy [ACQ|STATS];
                5 TrigLa(trig_la: Option<u8>) copy [ACQ|STATS];
                6 PatLa(pat_la: [Bit; 16]) copy [ACQ|STATS];
                7 TrigLevel(trig_level: f32) copy [ACQ|STATS];
                8 Slope(slope: Slope) copy [ACQ|STATS];
                9 TrigMode(trig_mode: TrigMode) copy [sweep];
                10 Holdoff(holdoff: f32) copy [ACQ|STATS];
                11 TrigType(trig_type: TrigType) copy [ACQ|STATS];
                12 TrigLevel2(trig_level2: f32) copy [ACQ|STATS];
                13 TrigNrej(trig_nrej: bool) copy [ACQ|STATS];
                14 AutoNrej(auto_nrej: bool) copy [NONE];
                15 TrigCoup(trig_coup: TrigCoup) copy [ACQ|STATS];
            }
            Trigger1 trigger1 {
                16 TrigSrc2(trig_src2: usize) copy [ACQ|STATS];
                17 TrigSlope2(trig_slope2: Slope) copy [ACQ|STATS];
                18 TrigPol(trig_pol: Pol) copy [ACQ|STATS];
                19 TrigWhen(trig_when: WhenKind) copy [ACQ|STATS];
                20 TrigT1(trig_t1: f32) copy [ACQ|STATS];
                21 TrigT2(trig_t2: f32) copy [ACQ|STATS];
                22 VideoStd(video_std: VideoStd) copy [ACQ|STATS];
                23 VideoSync(video_sync: VSync) copy [ACQ|STATS];
                24 VideoLine(video_line: u32) copy [ACQ|STATS];
                25 Pat(pat: [Bit; NCH]) copy [ACQ|STATS];
                26 PatLevels(pat_levels: [f32; NCH]) copy [ACQ|STATS];
                27 WinMode(win_mode: WinMode) copy [ACQ|STATS];
            }
            Trigger2 trigger2 {
                28 NthIdle(nth_idle: f32) copy [ACQ|STATS];
                29 NthN(nth_n: u32) copy [ACQ|STATS];
                30 Setup(setup: f32) copy [ACQ|STATS];
                31 Hold(hold: f32) copy [ACQ|STATS];
                105 TrigBus(trig_bus: BusWhat) copy [ACQ|STATS];
                106 TrigBusVal(trig_bus_val: u64) copy [ACQ|STATS];
                107 Ser(ser: SerialTrig) copy [ACQ|STATS];
                164 AutoFast(auto_fast: bool) copy [ACQ|STATS];
            }
            Acquire0 acquire0 {
                32 Run(run: RunState) copy [NONE];
                33 Status(status: TrigStatus) copy [NONE];
                34 Acq(acq: AcqMode) copy [ACQ|STATS|PHOSPHOR|HW_AVERAGE];
                35 AvgCount(avg_count: u32) copy [ACQ|STATS|PHOSPHOR|HW_AVERAGE];
                36 AvgHw(avg_hw: bool) copy [ACQ|STATS|PHOSPHOR|HW_AVERAGE];
                37 Segments(segments: usize) copy [ACQ|STATS|HISTORY];
                38 SegHw(seg_hw: bool) copy [ACQ|STATS|HISTORY];
                39 SegIvl(seg_ivl: f32) copy [ACQ|STATS|HISTORY];
                40 Mdepth(mdepth: usize) copy [ACQ|STATS];
                162 HwStamp(hw_stamp: u64) copy [NONE];
                163 HwAvgClear(hw_avg_clear: u64) copy [NONE];
                193 La(la: crate::la::LaSet) clone [NONE];
                243 Cores(cores: bool) copy [ACQ|STATS|PHOSPHOR];
            }
            Display0 display0 {
                41 Persist(persist: usize) copy [PHOSPHOR];
                42 Intensity(intensity: u32) copy [NONE];
                43 Grid(grid: Grid) copy [NONE];
                44 GridBright(grid_bright: u32) copy [NONE];
                45 GridFrame(grid_frame: bool) copy [NONE];
                46 WinLayout(win_layout: WinLayout) copy [VIEW];
                47 ChWin(ch_win: [usize; NCH]) copy [VIEW];
                48 MathWin(math_win: [usize; crate::math::NMATH]) copy [VIEW];
                49 LaWin(la_win: usize) copy [VIEW];
                88 Pane(pane: usize) copy [VIEW];
                65 TouchLock(touch_lock: bool) copy [NONE];
                121 ColorGrade(color_grade: bool) copy [PHOSPHOR];
            }
            Display1 display1 {
                122 Dots(dots: DotsMode) copy [NONE];
                123 Hdmi(hdmi: HdmiMode) copy [NONE];
                124 Views(views: usize) copy [NONE];
                125 ViewCur(view_cur: usize) copy [NONE];
                126 ViewSlots(view_slots: [String; VIEWS_MAX]) strings [NONE];
                127 ZSrc(z_src: Option<usize>) copy [ACQ|STATS];
                128 Hist(hist: Hist) copy [NONE];
                129 View(view: AView) copy [VIEW];
                192 TextSize(text_size: TextSize) copy [NONE];
            }
            Measure0 measure0 {
                50 Meas(meas: Vec<(Meas, WaveId)>) vec [STATS];
                51 Meas2(meas2: Vec<(usize, WaveId, WaveId)>) vec [STATS];
                52 DvmOn(dvm_on: bool) copy [ACQ|STATS];
                53 DvmSrc(dvm_src: usize) copy [ACQ|STATS];
                54 DvmMode(dvm_mode: usize) copy [ACQ|STATS];
                55 CntOn(cnt_on: bool) copy [ACQ|STATS];
                56 CntSrc(cnt_src: usize) copy [ACQ|STATS];
                57 CntMode(cnt_mode: usize) copy [ACQ|STATS];
                58 CntDigits(cnt_digits: u32) copy [ACQ|STATS];
                60 Cursor(cursor: CursorMode) copy [NONE];
                61 CurA(cur_a: f32) copy [NONE];
                62 CurB(cur_b: f32) copy [NONE];
            }
            Measure1 measure1 {
                63 CurC(cur_c: f32) copy [NONE];
                64 CurD(cur_d: f32) copy [NONE];
                89 MeasLabels(meas_labels: bool) copy [NONE];
                90 TrendSel(trend_sel: [usize; 2]) copy [VIEW];
                79 LogIvl(log_ivl: usize) copy [NONE];
            }
            Analysis0 analysis0 {
                72 Math(math: MathOp) copy [MATH|STATS];
                73 MathA(math_a: usize) copy [MATH|STATS];
                74 MathB(math_b: usize) copy [MATH|STATS];
                75 MathScale(math_scale: usize) copy [MATH|STATS];
                76 MathFc(math_fc: f32) copy [MATH|STATS];
                77 MathLin(math_lin: (f32, f32)) copy [MATH|STATS];
                78 MathExpr(math_expr: [String; crate::math::NMATH]) strings [MATH|STATS];
                80 SpecMargin(spec_margin: f32) copy [MATH|VIEW];
                81 MathOffset(math_offset: f32) copy [MATH|STATS];
                82 FftWin(fft_win: Window) copy [MATH|STATS];
                83 MathBw(math_bw: f32) copy [MATH|STATS];
                84 MathSel(math_sel: usize) copy [NONE];
            }
            Analysis1 analysis1 {
                85 Maths(maths: Box<[crate::math::MathCh; crate::math::NMATH]>) boxed_copy [MATH|STATS];
                86 Decs(decs: Box<[Option<DecCfg>; DECODERS]>) boxed_clone [NONE];
                87 DecSel(dec_sel: usize) copy [NONE];
                91 DecOn(dec_on: bool) copy [decoder];
                92 DecSrc(dec_src: usize) copy [decoder];
                93 DecBaud(dec_baud: usize) copy [decoder];
                94 DecCanrate(dec_canrate: usize) copy [decoder];
                95 DecCanh(dec_canh: bool) copy [decoder];
                96 DecFdrate(dec_fdrate: usize) copy [decoder];
                97 DecArate(dec_arate: usize) copy [decoder];
                98 DecTick(dec_tick: usize) copy [decoder];
                99 DecNibbles(dec_nibbles: u32) copy [decoder];
            }
            Analysis2 analysis2 {
                100 DecUspeed(dec_uspeed: usize) copy [decoder];
                101 DecSrc4(dec_src4: Option<usize>) copy [decoder];
                102 DecLaIn(dec_la_in: bool) copy [decoder];
                103 DecLa(dec_la: [Option<u8>; 4]) copy [decoder];
                104 DecDiff(dec_diff: Option<usize>) copy [decoder];
                108 DecThr(dec_thr: f32) copy [decoder];
                109 DecAscii(dec_ascii: bool) copy [NONE];
                110 DecProto(dec_proto: Proto) copy [decoder];
                111 DecSrc2(dec_src2: usize) copy [decoder];
                112 DecCs(dec_cs: Option<usize>) copy [decoder];
                113 DecMode(dec_mode: u8) copy [decoder];
                114 DecBits(dec_bits: u32) copy [decoder];
            }
            Analysis3 analysis3 {
                115 DecParity(dec_parity: oos_dsp::decode::Parity) copy [decoder];
                116 DecOpt(dec_opt: [u8; crate::protocols::OPTS]) copy [decoder];
                130 SpecSrc(spec_src: crate::model::WaveId) copy [MATH|VIEW];
                131 SpecCenter(spec_center: f64) copy [MATH|VIEW];
                132 SpecSpan(spec_span: f64) copy [MATH|VIEW];
                133 SpecRbw(spec_rbw: f64) copy [MATH|VIEW];
                134 SpecWin(spec_win: Window) copy [MATH|VIEW];
                135 SpecSort(spec_sort: usize) copy [NONE];
                136 SpecHarm(spec_harm: usize) copy [NONE];
                137 SpecTrace(spec_trace: usize) copy [MATH|VIEW];
                138 SpecRef(spec_ref: f32) copy [MATH|VIEW];
                139 SpecWf(spec_wf: bool) copy [MATH|VIEW];
            }
            Analysis4 analysis4 {
                140 SpecDist(spec_dist: bool) copy [MATH|VIEW];
                141 EyeSrc(eye_src: WaveId) copy [VIEW];
                142 EyeRate(eye_rate: f64) copy [VIEW];
                143 PwV(pw_v: WaveId) copy [VIEW];
                144 PwI(pw_i: WaveId) copy [VIEW];
                145 PwMode(pw_mode: PowerMode) copy [VIEW];
                146 BodeStart(bode_start: f64) copy [STATS|VIEW];
                147 BodeStop(bode_stop: f64) copy [STATS|VIEW];
                148 BodePoints(bode_points: usize) copy [STATS|VIEW];
                149 BodeAmp(bode_amp: f64) copy [STATS|VIEW];
                150 BodeIn(bode_in: WaveId) copy [STATS|VIEW];
                151 BodeOut(bode_out: WaveId) copy [STATS|VIEW];
            }
            Analysis5 analysis5 {
                152 BodeGen(bode_gen: usize) copy [STATS|VIEW];
                153 BodeAddr(bode_addr: String) string [STATS|VIEW];
                154 BodeUsb(bode_usb: bool) copy [STATS|VIEW];
                155 BodeSwTime(bode_sw_time: f64) copy [STATS|VIEW];
                156 BodeSwLog(bode_sw_log: bool) copy [STATS|VIEW];
                157 BodeSync(bode_sync: usize) copy [STATS|VIEW];
                158 BodeSyncSlope(bode_sync_slope: Slope) copy [STATS|VIEW];
                159 BodeSyncLevel(bode_sync_level: f32) copy [STATS|VIEW];
                160 BodeSwIn(bode_sw_in: usize) copy [STATS|VIEW];
                161 BodeRef(bode_ref: f64) copy [STATS|VIEW];
                165 BwSrc(bw_src: usize) copy [STATS|VIEW];
                166 BwHint(bw_hint: f64) copy [STATS|VIEW];
            }
            Analysis6 analysis6 {
                167 BwBins(bw_bins: usize) copy [STATS|VIEW];
                185 MaskOn(mask_on: bool) copy [STATS|MASK];
                186 Zones(zones: [crate::zone::Zone; 2]) zones [ACQ|STATS|ZONE];
                187 MaskSrc(mask_src: usize) copy [ACQ|STATS|MASK];
                188 MaskX(mask_x: f32) copy [MASK];
                189 MaskY(mask_y: f32) copy [MASK];
                190 MaskStop(mask_stop: bool) copy [NONE];
            }
            Interfaces0 interfaces0 {
                66 AuxOut(aux_out: AuxOut) copy [ACQ|STATS];
                168 GenSel(gen_sel: usize) copy [ACQ|STATS];
                169 GenBank(gen_bank: [GenCh; NGEN]) copy [ACQ|STATS];
                170 AfgOn(afg_on: bool) copy [ACQ|STATS];
                171 AfgArb(afg_arb: u32) copy [ACQ|STATS];
                172 AfgArbSrc(afg_arb_src: usize) copy [ACQ|STATS];
                173 AfgFreq(afg_freq: f64) copy [ACQ|STATS];
                174 AfgAmpl(afg_ampl: f64) copy [ACQ|STATS];
                175 AfgOffset(afg_offset: f64) copy [ACQ|STATS];
                176 AfgWave(afg_wave: usize) copy [ACQ|STATS];
                177 AfgDuty(afg_duty: f64) copy [ACQ|STATS];
                178 AfgSymmetry(afg_symmetry: f64) copy [ACQ|STATS];
            }
            Interfaces1 interfaces1 {
                179 AfgMod(afg_mod: usize) copy [ACQ|STATS];
                180 AfgModRate(afg_mod_rate: f64) copy [ACQ|STATS];
                181 AfgModDepth(afg_mod_depth: f64) copy [ACQ|STATS];
                182 AfgFmDev(afg_fm_dev: f64) copy [ACQ|STATS];
                183 AfgPmDev(afg_pm_dev: f64) copy [ACQ|STATS];
                184 AfgModShape(afg_mod_shape: usize) copy [ACQ|STATS];
                191 Quick(quick: Quick) copy [NONE];
            }
            ReferencesP0 references_p0 {
                244 DeskewCh(deskew_ch: [f64; NCH]) copy [STATS|PHOSPHOR];
                245 DeskewRef(deskew_ref: [f64; NREF]) copy [STATS];
                246 CurBSource(cur_b_source: Option<WaveId>) copy [NONE];
                247 TrigRef(trig_ref: Option<usize>) copy [ACQ|STATS];
                248 TrigRef2(trig_ref2: Option<usize>) copy [ACQ|STATS];
                249 Density(density: Density) copy [VIEW];
                250 RefWin(ref_win: [usize; NREF]) copy [VIEW];
                251 Cat(cat: Vec<(u16, WaveId, WaveId, WaveId, WaveId)>) vec [STATS];
                252 SplitPane(split_pane: usize) copy [VIEW];
                253 StripDock(strip_dock: StripDock) copy [VIEW];
                254 DecCphyRate(dec_cphy_rate: f64) copy [decoder];
                255 Bus(bus: TraceBus) bus [NONE];
            }
            ReferencesP1 references_p1 {
                256 MaskInv(mask_inv: bool) copy [STATS|MASK];
            }
            Analysis4P0 analysis4_p0 {
                257 EyeFromRecord(eye_from_record: bool) copy [VIEW];
                258 EyePlace(eye_place: EyePlace) copy [VIEW];
                259 EyeBer(eye_ber: f64) copy [VIEW];
                260 EyePnLo(eye_pn_lo: f64) copy [VIEW];
                261 EyePnHi(eye_pn_hi: f64) copy [VIEW];
                262 EyeNpj(eye_npj: bool) copy [VIEW];
                263 EyeB(eye_b: Option<WaveId>) copy [VIEW];
                264 EyeRail(eye_rail: Option<WaveId>) copy [VIEW];
                265 EyeNotch(eye_notch: f64) copy [VIEW];
                266 EyeHp(eye_hp: bool) copy [VIEW];
                267 PwFline(pw_fline: f64) copy [VIEW];
                268 PwFsw(pw_fsw: f64) copy [VIEW];
            }
            Analysis4P1 analysis4_p1 {
                269 PwVin(pw_vin: Option<WaveId>) copy [VIEW];
                270 PwClk(pw_clk: Option<WaveId>) copy [VIEW];
                271 PwRef(pw_ref: Option<f64>) copy [VIEW];
                272 BodeMode(bode_mode: oos_dsp::bode::Mode) copy [STATS|VIEW];
                273 SpecAcprW(spec_acpr_w: f64) copy [MATH|VIEW];
                274 SpecAcprS(spec_acpr_s: f64) copy [MATH|VIEW];
            }
            Measure1P0 measure1_p0 {
                275 MeasIdle(meas_idle: f64) copy [STATS];
                276 MeasOut(meas_out: oos_dsp::measure::Outside) copy [STATS];
                277 MeasN(meas_n: u32) copy [STATS];
                278 MeasData(meas_data: bool) copy [STATS];
                279 MeasLpct(meas_lpct: f64) copy [STATS];
                280 MeasHpct(meas_hpct: f64) copy [STATS];
                281 MeasFullRecord(meas_full_record: bool) copy [STATS];
                282 MeasTsetup(meas_tsetup: f64) copy [STATS];
                283 MeasThold(meas_thold: f64) copy [STATS];
            }
            Analysis5P0 analysis5_p0 {
                284 MagV(mag_v: WaveId) copy [VIEW];
                285 MagI(mag_i: WaveId) copy [VIEW];
                286 MagN(mag_n: f64) copy [VIEW];
                287 MagAe(mag_ae: f64) copy [VIEW];
                288 MagLe(mag_le: f64) copy [VIEW];
                289 MagRcu(mag_rcu: f64) copy [VIEW];
                290 MagPlot(mag_plot: MagPlot) copy [VIEW];
                291 MagCur(mag_cur: usize) copy [NONE];
                292 InvPage(inv_page: InvPage) copy [VIEW];
                293 InvA(inv_a: WaveId) copy [VIEW];
                294 InvB(inv_b: WaveId) copy [VIEW];
                295 InvC(inv_c: WaveId) copy [VIEW];
            }
            #[allow(
                clippy::enum_variant_names,
                reason = "one variant per Model field, named after the field and its shared prefix"
            )]
            Analysis5P1 analysis5_p1 {
                296 InvAng(inv_ang: InvAngle) copy [VIEW];
                297 InvPoles(inv_poles: u32) copy [VIEW];
                298 InvAn(inv_an: WaveId) copy [VIEW];
                299 InvVolt(inv_volt: bool) copy [VIEW];
                300 InvTq(inv_tq: Option<WaveId>) copy [VIEW];
                301 InvEv(inv_ev: Option<WaveId>) copy [VIEW];
                302 InvEi(inv_ei: Option<WaveId>) copy [VIEW];
                303 InvScale(inv_scale: f64) copy [VIEW];
                304 InvDegv(inv_degv: f64) copy [VIEW];
                305 InvOff(inv_off: f64) copy [VIEW];
                306 InvElec(inv_elec: bool) copy [VIEW];
                307 InvQa(inv_qa: u8) copy [VIEW];
            }
            Analysis5P2 analysis5_p2 {
                308 InvQb(inv_qb: u8) copy [VIEW];
                309 InvQi(inv_qi: Option<u8>) copy [VIEW];
                310 InvCpr(inv_cpr: f64) copy [VIEW];
                311 InvSwap(inv_swap: bool) copy [VIEW];
                312 InvHa(inv_ha: u8) copy [VIEW];
                313 InvHb(inv_hb: u8) copy [VIEW];
                314 InvHc(inv_hc: u8) copy [VIEW];
                315 DptV(dpt_v: WaveId) copy [VIEW];
                316 DptI(dpt_i: WaveId) copy [VIEW];
                317 DptG(dpt_g: WaveId) copy [VIEW];
                318 Dpt4th(dpt_4th: DptFourth) copy [VIEW];
                319 Dpt4ch(dpt_4ch: WaveId) copy [VIEW];
            }
            Analysis5P3 analysis5_p3 {
                320 DptPct(dpt_pct: f64) copy [VIEW];
                321 DptZoom(dpt_zoom: DptZoom) copy [VIEW];
            }
            #[allow(
                clippy::enum_variant_names,
                reason = "one variant per Model field, named after the field and its shared prefix"
            )]
            SimP0 sim_p0 {
                322 SimPreset(sim_preset: usize) copy [ACQ|STATS];
                323 SimSlot(sim_slot: usize) copy [ACQ|STATS];
                324 SimRate(sim_rate: f64) copy [ACQ|STATS];
                325 SimPrbs(sim_prbs: u8) copy [ACQ|STATS];
                326 SimVhigh(sim_vhigh: f64) copy [ACQ|STATS];
                327 SimVlow(sim_vlow: f64) copy [ACQ|STATS];
                328 SimRise(sim_rise: f64) copy [ACQ|STATS];
                329 SimNoise(sim_noise: f64) copy [ACQ|STATS];
                330 SimBw(sim_bw: f64) copy [ACQ|STATS];
                331 SimDeemph(sim_deemph: f64) copy [ACQ|STATS];
                332 SimSsc(sim_ssc: bool) copy [ACQ|STATS];
                333 SimSscCenter(sim_ssc_center: bool) copy [ACQ|STATS];
            }
            #[allow(
                clippy::enum_variant_names,
                reason = "one variant per Model field, named after the field and its shared prefix"
            )]
            SimP1 sim_p1 {
                334 SimSscDeviation(sim_ssc_deviation: f64) copy [ACQ|STATS];
                335 SimSscHz(sim_ssc_hz: f64) copy [ACQ|STATS];
                336 SimRj(sim_rj: f64) copy [ACQ|STATS];
                337 SimPj(sim_pj: f64) copy [ACQ|STATS];
                338 SimPjhz(sim_pjhz: f64) copy [ACQ|STATS];
                339 SimDcdPp(sim_dcd_pp: f64) copy [ACQ|STATS];
                340 SimCorrRj(sim_corr_rj: f64) copy [ACQ|STATS];
                341 SimCorrHz(sim_corr_hz: f64) copy [ACQ|STATS];
            }
            Display2P0 display2_p0 {
                342 DrawBatch(draw_batch: bool) copy [PHOSPHOR];
                343 BarCompact(bar_compact: bool) copy [NONE];
                344 HdmiView(hdmi_view: usize) copy [NONE];
            }
        }
    };
}
pub(super) use field_table;
